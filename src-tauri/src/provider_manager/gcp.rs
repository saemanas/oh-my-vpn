//! GCP Compute Engine cloud provider implementation.
//!
//! Implements `CloudProvider` for GCP using `google-cloud-compute-v1`.
//! See ADR-0002 (Rust SDK) and ADR-0005 (Provider Pricing API).
//!
//! ## GCP-specific design decisions
//!
//! - **Zone-scoped operations**: GCP Compute is zone-scoped, so server_id encodes
//!   project/zone/instance/firewall as compound IDs.
//! - **Deferred SSH key injection**: `create_ssh_key` caches key material internally
//!   because the trait method doesn't receive a zone. Actual metadata injection
//!   happens in `create_server` where the target zone is known.
//! - **Credentials format**: `api_key` stores a GCP service account JSON string.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use google_cloud_auth::credentials::service_account::Builder as SaBuilder;
use google_cloud_auth::credentials::Credentials;
use google_cloud_compute_v1::client::{Firewalls, Images, Instances, MachineTypes, Zones};
use google_cloud_compute_v1::model::access_config::Type as AccessConfigType;
use google_cloud_compute_v1::model::firewall::Allowed;
use google_cloud_compute_v1::model::metadata::Items;
use google_cloud_compute_v1::model::operation::Status as OperationStatus;
use google_cloud_compute_v1::model::{
    AccessConfig, AttachedDisk, AttachedDiskInitializeParams, Firewall, Instance, Metadata,
    NetworkInterface,
};
use google_cloud_compute_v1::model::zone;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::error::ProviderError;
use crate::types::{RegionInfo, ServerInfo, ServerStatus};

use super::CloudProvider;

// ── Internal types ──────────────────────────────────────────────────────────

/// Cached SSH key material awaiting injection during `create_server`.
struct PendingSshKey {
    label: String,
    public_key: String,
}

/// GCP Compute Engine cloud provider.
///
/// Holds internal caches for zone-to-machine-type mapping and pending SSH key
/// material.
pub struct GcpProvider {
    /// Zone → machine type cache, populated by `list_regions`.
    zone_machine_types: Arc<RwLock<HashMap<String, String>>>,
    /// Pending SSH key awaiting zone-scoped injection in `create_server`.
    pending_ssh_key: Arc<RwLock<Option<PendingSshKey>>>,
}

impl GcpProvider {
    /// Create a new GcpProvider with empty caches.
    pub fn new() -> Self {
        Self {
            zone_machine_types: Arc::new(RwLock::new(HashMap::new())),
            pending_ssh_key: Arc::new(RwLock::new(None)),
        }
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Parse a GCP service account JSON string and extract the `project_id`.
///
/// Returns `(parsed_json, project_id)` or `ProviderError::AuthInvalidKey`
/// if the input is not valid JSON or is missing the `project_id` field.
fn parse_gcp_credentials(api_key: &str) -> Result<(serde_json::Value, String), ProviderError> {
    let value: serde_json::Value = serde_json::from_str(api_key).map_err(|_| {
        ProviderError::AuthInvalidKey(
            "GCP credentials must be a valid service account JSON string".to_string(),
        )
    })?;

    let project_id = value
        .get("project_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ProviderError::AuthInvalidKey(
                "GCP service account JSON is missing the 'project_id' field".to_string(),
            )
        })?
        .to_string();

    Ok((value, project_id))
}

/// Build a `google_cloud_auth::credentials::Credentials` from a service account JSON value.
///
/// Uses the `google-cloud-auth` service account builder to construct credentials
/// that can be passed to compute client builders via `with_credentials()`.
fn build_credentials(sa_json: &serde_json::Value) -> Result<Credentials, ProviderError> {
    SaBuilder::new(sa_json.clone())
        .build()
        .map_err(|error| {
            ProviderError::AuthInvalidKey(format!(
                "GCP service account credentials are invalid: {:?}",
                error
            ))
        })
}

/// Map any GCP SDK error (Debug-printable) to a `ProviderError`.
///
/// Uses pattern matching on the debug string to identify common GCP error codes.
fn map_gcp_error<T: std::fmt::Debug>(error: T) -> ProviderError {
    let debug_string = format!("{:?}", error);

    // Check for timeout patterns
    if debug_string.contains("timed out")
        || debug_string.contains("Timeout")
        || debug_string.contains("DEADLINE_EXCEEDED")
    {
        return ProviderError::Timeout;
    }

    // Check for authentication errors (401 / UNAUTHENTICATED)
    if debug_string.contains("401")
        || debug_string.contains("Unauthenticated")
        || debug_string.contains("UNAUTHENTICATED")
    {
        return ProviderError::AuthInvalidKey(format!(
            "GCP authentication failed: {}",
            debug_string
        ));
    }

    // Check for permission errors (403 / PERMISSION_DENIED)
    if debug_string.contains("403")
        || debug_string.contains("PermissionDenied")
        || debug_string.contains("PERMISSION_DENIED")
    {
        return ProviderError::AuthInsufficientPermissions(format!(
            "GCP insufficient permissions: {}",
            debug_string
        ));
    }

    // Check for not found (404 / NOT_FOUND)
    if debug_string.contains("404")
        || debug_string.contains("NotFound")
        || debug_string.contains("NOT_FOUND")
    {
        return ProviderError::NotFound(format!("GCP resource not found: {}", debug_string));
    }

    // Check for rate limiting (429 / RESOURCE_EXHAUSTED)
    if debug_string.contains("429")
        || debug_string.contains("ResourceExhausted")
        || debug_string.contains("RESOURCE_EXHAUSTED")
    {
        return ProviderError::RateLimited {
            retry_after_seconds: 60,
        };
    }

    // Check for server errors (500 / 502 / 503 / INTERNAL)
    if debug_string.contains("500")
        || debug_string.contains("502")
        || debug_string.contains("503")
        || debug_string.contains("Internal")
        || debug_string.contains("INTERNAL")
    {
        return ProviderError::ServerError(format!("GCP server error: {}", debug_string));
    }

    ProviderError::Other(anyhow::anyhow!("GCP API error: {}", debug_string))
}

/// Parse compound server_id: `"{project_id}/{zone}/{instance_name}/{firewall_name}"`.
fn parse_compound_server_id(
    server_id: &str,
) -> Result<(&str, &str, &str, &str), ProviderError> {
    let parts: Vec<&str> = server_id.splitn(4, '/').collect();
    if parts.len() != 4 {
        return Err(ProviderError::Other(anyhow::anyhow!(
            "Invalid GCP server ID format '{}' -- expected 'project_id/zone/instance_name/firewall_name'",
            server_id
        )));
    }
    Ok((parts[0], parts[1], parts[2], parts[3]))
}

/// Map a GCP zone to a human-readable display name.
///
/// Format: `"{location} ({zone})"` for known zones.
/// Unknown zones are returned as-is.
fn get_zone_display_name(zone: &str) -> String {
    // Strip the zone suffix (e.g. "-a", "-b", "-c") to get the region.
    let region = zone.rsplit_once('-').map(|x| x.0).unwrap_or(zone);

    let location = match region {
        "us-central1" => "Iowa, US",
        "us-east1" => "South Carolina, US",
        "us-east4" => "N. Virginia, US",
        "us-west1" => "Oregon, US",
        "us-west4" => "Las Vegas, US",
        "europe-west1" => "Belgium, EU",
        "europe-west2" => "London, UK",
        "europe-west3" => "Frankfurt, DE",
        "europe-west4" => "Netherlands, EU",
        "europe-north1" => "Finland, EU",
        "asia-east1" => "Taiwan",
        "asia-east2" => "Hong Kong",
        "asia-northeast1" => "Tokyo, JP",
        "asia-northeast2" => "Osaka, JP",
        "asia-northeast3" => "Seoul, KR",
        "asia-south1" => "Mumbai, IN",
        "asia-southeast1" => "Singapore, SG",
        "australia-southeast1" => "Sydney, AU",
        "southamerica-east1" => "São Paulo, BR",
        "me-west1" => "Tel Aviv, IL",
        _ => return zone.to_string(),
    };

    format!("{} ({})", location, zone)
}

/// Convert a GCP instance status string to our `ServerStatus` enum.
fn map_instance_status(status: &str) -> ServerStatus {
    match status {
        "RUNNING" => ServerStatus::Running,
        "PROVISIONING" | "STAGING" => ServerStatus::Provisioning,
        "STOPPING" | "TERMINATED" | "SUSPENDING" | "SUSPENDED" => ServerStatus::Deleting,
        _ => ServerStatus::Provisioning,
    }
}

// ── CloudProvider implementation ────────────────────────────────────────────

#[async_trait]
impl CloudProvider for GcpProvider {
    async fn validate_credential(&self, api_key: &str) -> Result<(), ProviderError> {
        let (sa_json, project_id) = parse_gcp_credentials(api_key)?;
        let credentials = build_credentials(&sa_json)?;

        // Use Zones::list() as a lightweight liveness check -- no extra feature needed.
        let zones_client = Zones::builder()
            .with_credentials(credentials)
            .build()
            .await
            .map_err(map_gcp_error)?;

        zones_client
            .list()
            .set_project(&project_id)
            .set_max_results(1u32)
            .send()
            .await
            .map_err(map_gcp_error)?;

        Ok(())
    }

    async fn list_regions(&self, api_key: &str) -> Result<Vec<RegionInfo>, ProviderError> {
        const E2_MICRO_HOURLY_COST: f64 = 0.0084;
        const E2_MICRO_FILTER: &str = "name=e2-micro";

        let (sa_json, project_id) = parse_gcp_credentials(api_key)?;
        let credentials = build_credentials(&sa_json)?;

        // Build Zones client and fetch all zones for this project.
        let zones_client = Zones::builder()
            .with_credentials(credentials.clone())
            .build()
            .await
            .map_err(map_gcp_error)?;

        let zone_list = zones_client
            .list()
            .set_project(&project_id)
            .send()
            .await
            .map_err(map_gcp_error)?;

        // Build MachineTypes client for e2-micro availability checks.
        let mt_client = MachineTypes::builder()
            .with_credentials(credentials)
            .build()
            .await
            .map_err(map_gcp_error)?;

        let mut regions: Vec<RegionInfo> = Vec::new();
        let mut cache = self.zone_machine_types.write().await;
        cache.clear();

        for zone_obj in zone_list.items {
            let zone_name = match zone_obj.name {
                Some(ref n) => n.clone(),
                None => continue,
            };

            // Skip zones that are not UP.
            match &zone_obj.status {
                Some(zone::Status::Up) => {}
                Some(_) => continue,
                None => continue,
            }

            // Check whether e2-micro is available in this zone.
            let e2_available = mt_client
                .list()
                .set_project(&project_id)
                .set_zone(&zone_name)
                .set_filter(E2_MICRO_FILTER)
                .send()
                .await
                .map(|list| list.items.iter().any(|mt| mt.name.as_deref() == Some("e2-micro")))
                .unwrap_or(false);

            if !e2_available {
                continue;
            }

            cache.insert(zone_name.clone(), "e2-micro".to_string());

            regions.push(RegionInfo {
                region: zone_name.clone(),
                display_name: get_zone_display_name(&zone_name),
                instance_type: "e2-micro".to_string(),
                hourly_cost: E2_MICRO_HOURLY_COST,
            });
        }

        // Sort by cost ascending, then by zone name for deterministic ordering.
        regions.sort_by(|a, b| {
            a.hourly_cost
                .partial_cmp(&b.hourly_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.region.cmp(&b.region))
        });

        Ok(regions)
    }

    async fn create_ssh_key(
        &self,
        api_key: &str,
        public_key: &str,
        label: &str,
    ) -> Result<String, ProviderError> {
        // Validate credentials format (fail fast)
        let _ = parse_gcp_credentials(api_key)?;

        // Cache key material for deferred injection in create_server
        let mut pending = self.pending_ssh_key.write().await;
        *pending = Some(PendingSshKey {
            label: label.to_string(),
            public_key: public_key.to_string(),
        });

        // Return synthetic key_id -- actual injection happens in create_server
        Ok(format!("pending/{}", label))
    }

    async fn delete_ssh_key(
        &self,
        _api_key: &str,
        key_id: &str,
    ) -> Result<(), ProviderError> {
        if key_id.starts_with("pending/") {
            // Key was never injected -- just clear internal cache
            let mut pending = self.pending_ssh_key.write().await;
            *pending = None;
            return Ok(());
        }

        // GCP SSH keys are instance metadata -- "deletion" means the instance
        // is destroyed. Non-pending keys are cleaned up by destroy_server.
        Ok(())
    }

    async fn create_server(
        &self,
        api_key: &str,
        region: &str,
        _ssh_key_id: &str,
        cloud_init: &str,
    ) -> Result<ServerInfo, ProviderError> {
        let (sa_json, project_id) = parse_gcp_credentials(api_key)?;
        let credentials = build_credentials(&sa_json)?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // GCP `region` parameter is a zone (e.g., "us-central1-a")
        let zone = region;
        let resource_name = format!("oh-my-vpn-{}", timestamp);
        let instance_name = resource_name.clone();
        let firewall_name = resource_name.clone();

        // ── Step 1: Retrieve pending SSH key material ──────────────────────

        let public_key = {
            let pending = self.pending_ssh_key.read().await;
            pending
                .as_ref()
                .ok_or_else(|| {
                    ProviderError::Other(anyhow::anyhow!(
                        "No pending SSH key found -- call create_ssh_key first"
                    ))
                })?
                .public_key
                .clone()
        };

        // ── Step 2: Build GCP clients ──────────────────────────────────────

        let instances_client = Instances::builder()
            .with_credentials(credentials.clone())
            .build()
            .await
            .map_err(map_gcp_error)?;

        let firewalls_client = Firewalls::builder()
            .with_credentials(credentials.clone())
            .build()
            .await
            .map_err(map_gcp_error)?;

        let images_client = Images::builder()
            .with_credentials(credentials.clone())
            .build()
            .await
            .map_err(map_gcp_error)?;

        // ── Step 3: Resolve Ubuntu 24.04 LTS image ────────────────────────

        let image = images_client
            .get_from_family()
            .set_project("ubuntu-os-cloud")
            .set_family("ubuntu-2404-lts-amd64")
            .send()
            .await
            .map_err(map_gcp_error)?;

        let image_self_link = image.self_link.ok_or_else(|| {
            ProviderError::ProvisioningFailed(
                "GCP image response is missing self_link".to_string(),
            )
        })?;

        // ── Step 4: Create firewall rule (WireGuard UDP 51820) ─────────────

        let firewall = Firewall::new()
            .set_name(&firewall_name)
            .set_description("oh-my-vpn WireGuard UDP 51820 (ephemeral)")
            .set_network("global/networks/default")
            .set_allowed(vec![Allowed::new()
                .set_ip_protocol("udp")
                .set_ports(vec!["51820".to_string()])])
            .set_source_ranges(vec!["0.0.0.0/0".to_string()]);

        let fw_op = firewalls_client
            .insert()
            .set_project(&project_id)
            .set_body(firewall)
            .send()
            .await
            .map_err(map_gcp_error)?;

        // Poll firewall operation (global operation -- no zone required)
        let fw_op_name = fw_op.name.clone().unwrap_or_default();
        {
            let fw_max_wait = Duration::from_secs(60);
            let fw_start = std::time::Instant::now();
            loop {
                sleep(Duration::from_secs(3)).await;

                if fw_start.elapsed() > fw_max_wait {
                    let _ = firewalls_client
                        .delete()
                        .set_project(&project_id)
                        .set_firewall(&firewall_name)
                        .send()
                        .await;
                    return Err(ProviderError::ProvisioningFailed(
                        "Firewall creation timed out after 60s".to_string(),
                    ));
                }

                let op_result = firewalls_client
                    .get_operation()
                    .set_project(&project_id)
                    .set_operation(&fw_op_name)
                    .send()
                    .await;

                if let Ok(op) = op_result {
                    match op.status {
                        Some(OperationStatus::Done) => {
                            if let Some(error) = op.error {
                                let _ = firewalls_client
                                    .delete()
                                    .set_project(&project_id)
                                    .set_firewall(&firewall_name)
                                    .send()
                                    .await;
                                return Err(ProviderError::ProvisioningFailed(format!(
                                    "Firewall creation failed: {:?}",
                                    error
                                )));
                            }
                            break;
                        }
                        _ => continue,
                    }
                }
            }
        }

        // ── Step 5: Create instance ────────────────────────────────────────

        let disk_type_url = format!("zones/{}/diskTypes/pd-standard", zone);
        let machine_type_url = format!("zones/{}/machineTypes/e2-micro", zone);
        let ssh_metadata = format!("ubuntu:{}", public_key);

        let boot_disk = AttachedDisk::new()
            .set_boot(true)
            .set_auto_delete(true)
            .set_initialize_params(
                AttachedDiskInitializeParams::new()
                    .set_source_image(&image_self_link)
                    .set_disk_size_gb(20i64)
                    .set_disk_type(&disk_type_url),
            );

        let network_interface = NetworkInterface::new()
            .set_network("global/networks/default")
            .set_access_configs(vec![AccessConfig::new()
                .set_name("External NAT")
                .set_type(AccessConfigType::OneToOneNat)]);

        let metadata = Metadata::new().set_items(vec![
            Items::new()
                .set_key("startup-script")
                .set_value(cloud_init),
            Items::new()
                .set_key("ssh-keys")
                .set_value(ssh_metadata),
        ]);

        let instance = Instance::new()
            .set_name(&instance_name)
            .set_machine_type(&machine_type_url)
            .set_disks(vec![boot_disk])
            .set_network_interfaces(vec![network_interface])
            .set_metadata(metadata);

        let inst_op = match instances_client
            .insert()
            .set_project(&project_id)
            .set_zone(zone)
            .set_body(instance)
            .send()
            .await
        {
            Ok(op) => op,
            Err(error) => {
                // Cleanup: delete firewall (best-effort)
                let _ = firewalls_client
                    .delete()
                    .set_project(&project_id)
                    .set_firewall(&firewall_name)
                    .send()
                    .await;
                return Err(map_gcp_error(error));
            }
        };

        // ── Step 6: Poll until instance operation is Done ─────────────────

        let inst_op_name = inst_op.name.clone().unwrap_or_default();
        let max_wait = Duration::from_secs(120);
        let poll_start = std::time::Instant::now();
        let mut attempt: u32 = 0;

        loop {
            let backoff_ms = 3000u64
                .saturating_mul(2u64.saturating_pow(attempt))
                .min(15000);
            sleep(Duration::from_millis(backoff_ms)).await;
            attempt += 1;

            if poll_start.elapsed() > max_wait {
                // Cleanup: delete instance + firewall (best-effort)
                let _ = instances_client
                    .delete()
                    .set_project(&project_id)
                    .set_zone(zone)
                    .set_instance(&instance_name)
                    .send()
                    .await;
                let _ = firewalls_client
                    .delete()
                    .set_project(&project_id)
                    .set_firewall(&firewall_name)
                    .send()
                    .await;
                return Err(ProviderError::ProvisioningFailed(format!(
                    "Instance {} did not reach Running status within {}s",
                    instance_name,
                    max_wait.as_secs()
                )));
            }

            let op_result = instances_client
                .get_operation()
                .set_project(&project_id)
                .set_zone(zone)
                .set_operation(&inst_op_name)
                .send()
                .await;

            if let Ok(op) = op_result {
                match op.status {
                    Some(OperationStatus::Done) => {
                        if let Some(error) = op.error {
                            // Cleanup: delete instance + firewall (best-effort)
                            let _ = instances_client
                                .delete()
                                .set_project(&project_id)
                                .set_zone(zone)
                                .set_instance(&instance_name)
                                .send()
                                .await;
                            let _ = firewalls_client
                                .delete()
                                .set_project(&project_id)
                                .set_firewall(&firewall_name)
                                .send()
                                .await;
                            return Err(ProviderError::ProvisioningFailed(format!(
                                "Instance creation failed: {:?}",
                                error
                            )));
                        }
                        break;
                    }
                    _ => continue,
                }
            }
        }

        // ── Step 7: Fetch instance details for public IP ───────────────────

        let instance_info = match instances_client
            .get()
            .set_project(&project_id)
            .set_zone(zone)
            .set_instance(&instance_name)
            .send()
            .await
        {
            Ok(info) => info,
            Err(error) => {
                // Cleanup: delete instance + firewall (best-effort)
                let _ = instances_client
                    .delete()
                    .set_project(&project_id)
                    .set_zone(zone)
                    .set_instance(&instance_name)
                    .send()
                    .await;
                let _ = firewalls_client
                    .delete()
                    .set_project(&project_id)
                    .set_firewall(&firewall_name)
                    .send()
                    .await;
                return Err(map_gcp_error(error));
            }
        };

        let public_ip = instance_info
            .network_interfaces
            .first()
            .and_then(|ni| ni.access_configs.first())
            .and_then(|ac| ac.nat_ip.clone())
            .unwrap_or_default();

        // Clear pending SSH key after successful use
        {
            let mut pending = self.pending_ssh_key.write().await;
            *pending = None;
        }

        Ok(ServerInfo {
            server_id: format!(
                "{}/{}/{}/{}",
                project_id, zone, instance_name, firewall_name
            ),
            public_ip,
            status: ServerStatus::Running,
        })
    }

    async fn destroy_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<(), ProviderError> {
        let (project_id, zone, instance_name, firewall_name) =
            parse_compound_server_id(server_id)?;
        let (sa_json, _) = parse_gcp_credentials(api_key)?;
        let credentials = build_credentials(&sa_json)?;

        let instances_client = Instances::builder()
            .with_credentials(credentials.clone())
            .build()
            .await
            .map_err(map_gcp_error)?;

        let firewalls_client = Firewalls::builder()
            .with_credentials(credentials)
            .build()
            .await
            .map_err(map_gcp_error)?;

        // ── Step 1: Delete instance (zone-scoped operation) ────────────────

        let inst_op = instances_client
            .delete()
            .set_project(project_id)
            .set_zone(zone)
            .set_instance(instance_name)
            .send()
            .await
            .map_err(map_gcp_error)?;

        // Poll until instance delete operation is Done (or timeout -- proceed anyway)
        let inst_op_name = inst_op.name.clone().unwrap_or_default();
        let max_wait = Duration::from_secs(120);
        let poll_start = std::time::Instant::now();

        loop {
            sleep(Duration::from_secs(5)).await;

            if poll_start.elapsed() > max_wait {
                // Timeout -- proceed to firewall cleanup best-effort
                break;
            }

            let op_result = instances_client
                .get_operation()
                .set_project(project_id)
                .set_zone(zone)
                .set_operation(&inst_op_name)
                .send()
                .await;

            if let Ok(op) = op_result {
                match op.status {
                    Some(OperationStatus::Done) => break,
                    _ => continue,
                }
            }
        }

        // ── Step 2: Delete firewall (global, best-effort with retries) ─────

        // Brief wait for GCP to release instance-held network resources
        sleep(Duration::from_secs(3)).await;

        for attempt in 0..3u64 {
            match firewalls_client
                .delete()
                .set_project(project_id)
                .set_firewall(firewall_name)
                .send()
                .await
            {
                Ok(_) => break,
                Err(_) if attempt < 2 => {
                    sleep(Duration::from_secs(5 * (attempt + 1))).await;
                }
                Err(_) => break, // Best-effort -- skip silently on final failure
            }
        }

        Ok(())
    }

    async fn get_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<Option<ServerInfo>, ProviderError> {
        let (project_id, zone, instance_name, _firewall_name) =
            parse_compound_server_id(server_id)?;
        let (sa_json, _) = parse_gcp_credentials(api_key)?;
        let credentials = build_credentials(&sa_json)?;

        let instances_client = Instances::builder()
            .with_credentials(credentials)
            .build()
            .await
            .map_err(map_gcp_error)?;

        let result = instances_client
            .get()
            .set_project(project_id)
            .set_zone(zone)
            .set_instance(instance_name)
            .send()
            .await;

        match result {
            Ok(instance) => {
                // Extract status string from the SDK enum via .name()
                let status_str = instance
                    .status
                    .as_ref()
                    .and_then(|s| s.name())
                    .unwrap_or("UNKNOWN");

                // GCP keeps TERMINATED instances visible -- treat as gone (same as AWS)
                if status_str == "TERMINATED" {
                    return Ok(None);
                }

                let server_status = map_instance_status(status_str);

                let public_ip = instance
                    .network_interfaces
                    .first()
                    .and_then(|ni| ni.access_configs.first())
                    .and_then(|ac| ac.nat_ip.clone())
                    .unwrap_or_default();

                Ok(Some(ServerInfo {
                    server_id: server_id.to_string(),
                    public_ip,
                    status: server_status,
                }))
            }
            Err(error) => {
                // 404 / NotFound -- instance gone, not an error for orphan detection
                let error_string = format!("{:?}", error);
                if error_string.contains("404")
                    || error_string.contains("NotFound")
                    || error_string.contains("NOT_FOUND")
                {
                    return Ok(None);
                }
                Err(map_gcp_error(error))
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_gcp_credentials_valid() {
        let sa_json = serde_json::json!({
            "type": "service_account",
            "project_id": "my-project-123",
            "private_key_id": "key-id",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----\n",
            "client_email": "sa@my-project-123.iam.gserviceaccount.com",
            "client_id": "123456789",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token"
        });

        let (parsed, project_id) =
            parse_gcp_credentials(&sa_json.to_string()).unwrap();

        assert_eq!(project_id, "my-project-123");
        assert_eq!(
            parsed.get("client_email").and_then(|v| v.as_str()),
            Some("sa@my-project-123.iam.gserviceaccount.com")
        );
    }

    #[test]
    fn test_parse_gcp_credentials_invalid_json() {
        let result = parse_gcp_credentials("not-valid-json");
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[test]
    fn test_parse_gcp_credentials_missing_project_id() {
        let sa_json = serde_json::json!({
            "type": "service_account",
            "client_email": "sa@example.iam.gserviceaccount.com"
        });
        let result = parse_gcp_credentials(&sa_json.to_string());
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[test]
    fn test_parse_compound_server_id_valid() {
        let (project, zone, instance, firewall) =
            parse_compound_server_id("my-project/us-central1-a/my-instance/my-firewall")
                .unwrap();
        assert_eq!(project, "my-project");
        assert_eq!(zone, "us-central1-a");
        assert_eq!(instance, "my-instance");
        assert_eq!(firewall, "my-firewall");
    }

    #[test]
    fn test_parse_compound_server_id_invalid() {
        // Only 3 parts -- missing firewall name
        let result = parse_compound_server_id("my-project/us-central1-a/my-instance");
        assert!(result.is_err());

        // No slashes at all
        let result = parse_compound_server_id("no-slashes");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_zone_display_name_known() {
        assert_eq!(
            get_zone_display_name("us-central1-a"),
            "Iowa, US (us-central1-a)"
        );
        assert_eq!(
            get_zone_display_name("europe-west1-b"),
            "Belgium, EU (europe-west1-b)"
        );
        assert_eq!(
            get_zone_display_name("asia-east1-a"),
            "Taiwan (asia-east1-a)"
        );
        assert_eq!(
            get_zone_display_name("asia-northeast1-b"),
            "Tokyo, JP (asia-northeast1-b)"
        );
        assert_eq!(
            get_zone_display_name("australia-southeast1-c"),
            "Sydney, AU (australia-southeast1-c)"
        );
        assert_eq!(
            get_zone_display_name("southamerica-east1-a"),
            "São Paulo, BR (southamerica-east1-a)"
        );
    }

    #[test]
    fn test_get_zone_display_name_unknown() {
        assert_eq!(
            get_zone_display_name("xx-unknown1-z"),
            "xx-unknown1-z"
        );
        assert_eq!(get_zone_display_name("custom-zone"), "custom-zone");
    }

    #[test]
    fn test_map_instance_status() {
        assert_eq!(map_instance_status("RUNNING"), ServerStatus::Running);
        assert_eq!(map_instance_status("PROVISIONING"), ServerStatus::Provisioning);
        assert_eq!(map_instance_status("STAGING"), ServerStatus::Provisioning);
        assert_eq!(map_instance_status("STOPPING"), ServerStatus::Deleting);
        assert_eq!(map_instance_status("TERMINATED"), ServerStatus::Deleting);
        assert_eq!(map_instance_status("SUSPENDING"), ServerStatus::Deleting);
        assert_eq!(map_instance_status("SUSPENDED"), ServerStatus::Deleting);
        // Unknown defaults to Provisioning
        assert_eq!(map_instance_status("UNKNOWN_STATE"), ServerStatus::Provisioning);
    }

    #[test]
    fn test_gcp_provider_new() {
        let _provider = GcpProvider::new();
        // Verifies construction succeeds without panic
    }

    #[tokio::test]
    async fn test_create_ssh_key_returns_pending_id() {
        let provider = GcpProvider::new();
        let sa_json = serde_json::json!({
            "type": "service_account",
            "project_id": "my-project-123",
            "private_key_id": "key-id",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----\n",
            "client_email": "sa@my-project-123.iam.gserviceaccount.com",
            "client_id": "123456789",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token"
        });

        let key_id = provider
            .create_ssh_key(
                &sa_json.to_string(),
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAATEST test@test",
                "oh-my-vpn-test",
            )
            .await
            .unwrap();
        assert_eq!(key_id, "pending/oh-my-vpn-test");

        // Verify key was cached
        let pending = provider.pending_ssh_key.read().await;
        assert!(pending.is_some());
        assert_eq!(pending.as_ref().unwrap().label, "oh-my-vpn-test");
    }

    #[tokio::test]
    async fn test_delete_pending_ssh_key_clears_cache() {
        let provider = GcpProvider::new();
        // Set up a pending key
        {
            let mut pending = provider.pending_ssh_key.write().await;
            *pending = Some(PendingSshKey {
                label: "test".to_string(),
                public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAATEST test@test".to_string(),
            });
        }

        // Delete with pending/ prefix should clear cache without API call
        provider
            .delete_ssh_key("unused", "pending/test")
            .await
            .unwrap();

        let pending = provider.pending_ssh_key.read().await;
        assert!(pending.is_none());
    }

    #[tokio::test]
    async fn test_delete_non_pending_ssh_key_succeeds() {
        let provider = GcpProvider::new();
        // Non-pending key deletion is a no-op for GCP (instance metadata cleanup)
        let result = provider
            .delete_ssh_key("unused", "my-project/us-central1-a/label")
            .await;
        assert!(result.is_ok());
    }

    // ── Integration tests (require GCP_TEST_CREDENTIALS env var) ────────

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_valid() {
        let api_key = std::env::var("GCP_TEST_CREDENTIALS")
            .expect("GCP_TEST_CREDENTIALS must be set (service account JSON string)");
        let provider = GcpProvider::new();
        let result = provider.validate_credential(&api_key).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_invalid() {
        let provider = GcpProvider::new();
        let invalid_sa = serde_json::json!({
            "type": "service_account",
            "project_id": "nonexistent-project-12345",
            "private_key_id": "fake-key-id",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0Z3VS5JJcds3xfn/ygWyF8PbnGy0AHB7MhgHcTz6sE2I2yPB\naOl3JCxPP9KmEJr4GlNaLBMpnqGBx6LKHA==\n-----END RSA PRIVATE KEY-----\n",
            "client_email": "fake@nonexistent-project-12345.iam.gserviceaccount.com",
            "client_id": "000000000000000000000",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token"
        });
        let result = provider.validate_credential(&invalid_sa.to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_regions() {
        let api_key = std::env::var("GCP_TEST_CREDENTIALS")
            .expect("GCP_TEST_CREDENTIALS must be set (service account JSON string)");
        let provider = GcpProvider::new();
        let regions = provider.list_regions(&api_key).await.unwrap();

        assert!(!regions.is_empty());
        // Verify sorted by cost ascending (all same cost for e2-micro)
        for window in regions.windows(2) {
            assert!(window[0].hourly_cost <= window[1].hourly_cost);
        }
        // Verify all instance types are e2-micro
        for region in &regions {
            assert_eq!(region.instance_type, "e2-micro");
        }
        // Verify cache was populated
        let cache = provider.zone_machine_types.read().await;
        assert!(!cache.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_server_create_destroy() {
        let api_key = std::env::var("GCP_TEST_CREDENTIALS")
            .expect("GCP_TEST_CREDENTIALS must be set (service account JSON string)");
        let provider = GcpProvider::new();

        // Populate zone cache
        let regions = provider.list_regions(&api_key).await.unwrap();
        assert!(!regions.is_empty());
        let zone = &regions[0].region;

        // Create SSH key (deferred)
        let public_key =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl test@oh-my-vpn";
        let key_id = provider
            .create_ssh_key(&api_key, public_key, "oh-my-vpn-test-key")
            .await
            .unwrap();

        // Create server
        let cloud_init = "#!/bin/bash\necho 'test'";
        let server_info = provider
            .create_server(&api_key, zone, &key_id, cloud_init)
            .await
            .unwrap();

        assert!(!server_info.public_ip.is_empty());
        assert_eq!(server_info.status, ServerStatus::Running);

        // Verify server exists
        let check = provider
            .get_server(&api_key, &server_info.server_id)
            .await
            .unwrap();
        assert!(check.is_some());

        // Destroy server
        provider
            .destroy_server(&api_key, &server_info.server_id)
            .await
            .unwrap();

        // Wait and verify destruction
        sleep(Duration::from_secs(15)).await;
        let check = provider
            .get_server(&api_key, &server_info.server_id)
            .await
            .unwrap();
        assert!(check.is_none());
    }
}
