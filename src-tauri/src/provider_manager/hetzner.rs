//! Hetzner Cloud provider implementation.
//!
//! Implements `CloudProvider` for Hetzner Cloud using the `hcloud` crate.
//! See ADR-0002 (Rust SDK) and ADR-0005 (Provider Pricing API).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use hcloud::apis::configuration::Configuration;
use hcloud::apis::locations_api;
use hcloud::apis::pricing_api;
use hcloud::apis::servers_api;
use hcloud::apis::ssh_keys_api;
use hcloud::apis::Error as HcloudError;
use hcloud::models::server::Status as HcloudServerStatus;
use hcloud::models::{CreateServerRequest, CreateSshKeyRequest};
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::error::ProviderError;
use crate::types::{RegionInfo, ServerInfo, ServerStatus};

use super::CloudProvider;

/// Hetzner Cloud provider.
///
/// Holds an internal cache of region → cheapest server type mapping,
/// populated by `list_regions` and consumed by `create_server`.
pub struct HetznerProvider {
    region_server_types: Arc<RwLock<HashMap<String, String>>>,
}

impl HetznerProvider {
    /// Create a new HetznerProvider with an empty region-to-server-type cache.
    pub fn new() -> Self {
        Self {
            region_server_types: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// Build an `hcloud::Configuration` with the given API key.
///
/// Creates a fresh configuration per call -- keys come from Keychain
/// and are never cached in the provider struct (ADR-0004).
fn make_configuration(api_key: &str) -> Configuration {
    Configuration {
        bearer_access_token: Some(api_key.to_string()),
        ..Configuration::default()
    }
}

/// Map an `hcloud::apis::Error` to a `ProviderError`.
///
/// Inspects HTTP status codes and maps to the appropriate error variant.
fn map_hcloud_error<T: std::fmt::Debug>(error: HcloudError<T>) -> ProviderError {
    match &error {
        HcloudError::ResponseError(response) => match response.status.as_u16() {
            401 => ProviderError::AuthInvalidKey(format!(
                "Hetzner API authentication failed: {}",
                response.status
            )),
            403 => ProviderError::AuthInsufficientPermissions(format!(
                "Hetzner API insufficient permissions: {}",
                response.status
            )),
            404 => ProviderError::NotFound(format!(
                "Hetzner resource not found: {}",
                response.status
            )),
            429 => ProviderError::RateLimited {
                retry_after_seconds: 30,
            },
            status if status >= 500 => ProviderError::ServerError(format!(
                "Hetzner server error: {}",
                response.status
            )),
            _ => ProviderError::Other(anyhow::anyhow!(
                "Hetzner API error: {:?}",
                error
            )),
        },
        HcloudError::Reqwest(reqwest_error) => {
            if reqwest_error.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::Other(anyhow::anyhow!(
                    "Hetzner request error: {}",
                    reqwest_error
                ))
            }
        }
        _ => ProviderError::Other(anyhow::anyhow!(
            "Hetzner API error: {:?}",
            error
        )),
    }
}

/// Parse a Hetzner pricing string (e.g., `"0.0048000000000000"`) to `f64`.
///
/// Returns `f64::MAX` on parse failure so unparseable entries sort last.
fn parse_price(price_string: &str) -> f64 {
    price_string.parse::<f64>().unwrap_or(f64::MAX)
}

/// Convert a Hetzner `server::Status` enum to our `ServerStatus` enum.
fn map_server_status(status: &HcloudServerStatus) -> ServerStatus {
    match status {
        HcloudServerStatus::Running => ServerStatus::Running,
        HcloudServerStatus::Deleting => ServerStatus::Deleting,
        // Initializing, Starting, Off, Stopping, Migrating, Rebuilding, Unknown
        _ => ServerStatus::Provisioning,
    }
}

#[async_trait]
impl CloudProvider for HetznerProvider {
    async fn validate_credential(&self, api_key: &str) -> Result<(), ProviderError> {
        let config = make_configuration(api_key);
        servers_api::list_servers(
            &config,
            servers_api::ListServersParams::default(),
        )
        .await
        .map_err(map_hcloud_error)?;
        Ok(())
    }

    async fn list_regions(&self, api_key: &str) -> Result<Vec<RegionInfo>, ProviderError> {
        let config = make_configuration(api_key);

        // 1. Get pricing for all server types per location
        let pricing_response = pricing_api::list_prices(&config)
            .await
            .map_err(map_hcloud_error)?;

        let pricing = pricing_response.pricing;
        let server_types = pricing.server_types;

        // 2. Get locations for display names
        let locations_response = locations_api::list_locations(
            &config,
            locations_api::ListLocationsParams::default(),
        )
        .await
        .map_err(map_hcloud_error)?;

        let locations = locations_response.locations;

        // Build location code → (city, country) map
        let mut location_display: HashMap<String, (String, String)> = HashMap::new();
        for location in &locations {
            location_display.insert(
                location.name.clone(),
                (location.city.clone(), location.country.to_uppercase()),
            );
        }

        // 3. For each location, find the cheapest shared server type
        let mut cheapest_per_location: HashMap<String, (String, f64)> = HashMap::new();

        for server_type in &server_types {
            let type_name = &server_type.name;

            for price_entry in &server_type.prices {
                let location_name = &price_entry.location;
                let hourly_cost = parse_price(&price_entry.price_hourly.gross);

                // Skip if cost is unparseable (f64::MAX)
                if hourly_cost == f64::MAX {
                    continue;
                }

                let is_cheaper = match cheapest_per_location.get(location_name.as_str()) {
                    Some((_, existing_cost)) => hourly_cost < *existing_cost,
                    None => true,
                };

                if is_cheaper {
                    cheapest_per_location
                        .insert(location_name.clone(), (type_name.clone(), hourly_cost));
                }
            }
        }

        // 4. Build RegionInfo list and populate cache
        let mut regions: Vec<RegionInfo> = Vec::new();
        let mut cache_update: HashMap<String, String> = HashMap::new();

        for (location, (server_type, hourly_cost)) in &cheapest_per_location {
            let (city, country) = location_display
                .get(location)
                .cloned()
                .unwrap_or_else(|| (location.clone(), String::new()));

            let display_name = if country.is_empty() {
                city.clone()
            } else {
                format!("{}, {}", city, country)
            };

            regions.push(RegionInfo {
                region: location.clone(),
                display_name,
                instance_type: server_type.clone(),
                hourly_cost: *hourly_cost,
            });

            cache_update.insert(location.clone(), server_type.clone());
        }

        // Sort by hourly_cost ascending
        regions.sort_by(|a, b| {
            a.hourly_cost
                .partial_cmp(&b.hourly_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 5. Populate the region_server_types cache
        {
            let mut cache = self.region_server_types.write().await;
            *cache = cache_update;
        }

        Ok(regions)
    }

    async fn create_ssh_key(
        &self,
        api_key: &str,
        public_key: &str,
        label: &str,
    ) -> Result<String, ProviderError> {
        let config = make_configuration(api_key);
        let request = CreateSshKeyRequest {
            name: label.to_string(),
            public_key: public_key.to_string(),
            labels: None,
        };
        let response = ssh_keys_api::create_ssh_key(
            &config,
            ssh_keys_api::CreateSshKeyParams {
                create_ssh_key_request: request,
            },
        )
        .await
        .map_err(map_hcloud_error)?;

        Ok(response.ssh_key.id.to_string())
    }

    async fn delete_ssh_key(
        &self,
        api_key: &str,
        key_id: &str,
    ) -> Result<(), ProviderError> {
        let config = make_configuration(api_key);
        let id = key_id.parse::<i64>().map_err(|error| {
            ProviderError::Other(anyhow::anyhow!(
                "Invalid Hetzner SSH key ID '{}': {}",
                key_id,
                error
            ))
        })?;
        ssh_keys_api::delete_ssh_key(
            &config,
            ssh_keys_api::DeleteSshKeyParams { id },
        )
        .await
        .map_err(map_hcloud_error)?;
        Ok(())
    }

    async fn create_server(
        &self,
        api_key: &str,
        region: &str,
        ssh_key_id: &str,
        cloud_init: &str,
    ) -> Result<ServerInfo, ProviderError> {
        let config = make_configuration(api_key);

        // Resolve server type from cache; on miss, call list_regions to populate
        let server_type = {
            let cache = self.region_server_types.read().await;
            cache.get(region).cloned()
        };

        let server_type = match server_type {
            Some(st) => st,
            None => {
                // Cache miss -- populate by calling list_regions
                self.list_regions(api_key).await?;
                let cache = self.region_server_types.read().await;
                cache.get(region).cloned().ok_or_else(|| {
                    ProviderError::Other(anyhow::anyhow!(
                        "No server type found for region '{}'",
                        region
                    ))
                })?
            }
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let request = CreateServerRequest {
            image: "ubuntu-24.04".to_string(),
            name: format!("oh-my-vpn-{}", timestamp),
            server_type,
            location: Some(region.to_string()),
            ssh_keys: Some(vec![ssh_key_id.to_string()]),
            user_data: Some(cloud_init.to_string()),
            start_after_create: Some(true),
            automount: None,
            datacenter: None,
            firewalls: None,
            labels: None,
            networks: None,
            placement_group: None,
            public_net: None,
            volumes: None,
        };

        let create_response = servers_api::create_server(
            &config,
            servers_api::CreateServerParams {
                create_server_request: request,
            },
        )
        .await
        .map_err(map_hcloud_error)?;

        let server_id = create_response.server.id;

        // Poll until server reaches Running status (max 120s, exponential backoff)
        // Starts at 3s, doubles each iteration, capped at 15s per interval.
        let max_wait = Duration::from_secs(120);
        let initial_interval_milliseconds: u64 = 3000;
        let max_interval_milliseconds: u64 = 15000;
        let start = std::time::Instant::now();
        let mut attempt: u32 = 0;

        loop {
            let backoff_milliseconds = initial_interval_milliseconds
                .saturating_mul(2u64.saturating_pow(attempt))
                .min(max_interval_milliseconds);
            sleep(Duration::from_millis(backoff_milliseconds)).await;
            attempt += 1;

            if start.elapsed() > max_wait {
                return Err(ProviderError::ProvisioningFailed(format!(
                    "Server {} did not reach Running status within {}s",
                    server_id,
                    max_wait.as_secs()
                )));
            }

            let get_response = servers_api::get_server(
                &config,
                servers_api::GetServerParams { id: server_id },
            )
            .await
            .map_err(map_hcloud_error)?;

            if let Some(ref server) = get_response.server {
                if map_server_status(&server.status) == ServerStatus::Running {
                    let public_ip = server
                        .public_net
                        .ipv4
                        .as_ref()
                        .map(|ipv4| ipv4.ip.clone())
                        .unwrap_or_default();

                    return Ok(ServerInfo {
                        server_id: server_id.to_string(),
                        public_ip,
                        status: ServerStatus::Running,
                    });
                }
            }
        }
    }

    async fn destroy_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<(), ProviderError> {
        let config = make_configuration(api_key);
        let id = server_id.parse::<i64>().map_err(|error| {
            ProviderError::Other(anyhow::anyhow!(
                "Invalid Hetzner server ID '{}': {}",
                server_id,
                error
            ))
        })?;
        servers_api::delete_server(
            &config,
            servers_api::DeleteServerParams { id },
        )
        .await
        .map_err(map_hcloud_error)?;
        Ok(())
    }

    async fn get_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<Option<ServerInfo>, ProviderError> {
        let config = make_configuration(api_key);
        let id = server_id.parse::<i64>().map_err(|error| {
            ProviderError::Other(anyhow::anyhow!(
                "Invalid Hetzner server ID '{}': {}",
                server_id,
                error
            ))
        })?;

        match servers_api::get_server(
            &config,
            servers_api::GetServerParams { id },
        )
        .await
        {
            Ok(response) => {
                let server_info = response.server.map(|server| {
                    let public_ip = server
                        .public_net
                        .ipv4
                        .as_ref()
                        .map(|ipv4| ipv4.ip.clone())
                        .unwrap_or_default();

                    ServerInfo {
                        server_id: server_id.to_string(),
                        public_ip,
                        status: map_server_status(&server.status),
                    }
                });
                Ok(server_info)
            }
            Err(HcloudError::ResponseError(ref response)) if response.status == 404 => {
                Ok(None)
            }
            Err(error) => Err(map_hcloud_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_price_valid() {
        assert!((parse_price("0.0048000000000000") - 0.0048).abs() < f64::EPSILON);
        assert!((parse_price("1.23") - 1.23).abs() < f64::EPSILON);
        assert!((parse_price("0") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_price_invalid() {
        assert_eq!(parse_price("invalid"), f64::MAX);
        assert_eq!(parse_price(""), f64::MAX);
    }

    #[test]
    fn test_map_server_status() {
        assert_eq!(map_server_status(&HcloudServerStatus::Running), ServerStatus::Running);
        assert_eq!(map_server_status(&HcloudServerStatus::Deleting), ServerStatus::Deleting);
        assert_eq!(map_server_status(&HcloudServerStatus::Initializing), ServerStatus::Provisioning);
        assert_eq!(map_server_status(&HcloudServerStatus::Starting), ServerStatus::Provisioning);
        assert_eq!(map_server_status(&HcloudServerStatus::Off), ServerStatus::Provisioning);
        assert_eq!(map_server_status(&HcloudServerStatus::Stopping), ServerStatus::Provisioning);
        assert_eq!(map_server_status(&HcloudServerStatus::Unknown), ServerStatus::Provisioning);
    }

    #[test]
    fn test_server_id_conversion() {
        let hcloud_id: i64 = 108532637;
        let string_id = hcloud_id.to_string();
        assert_eq!(string_id, "108532637");
        let back: i64 = string_id.parse().unwrap();
        assert_eq!(back, hcloud_id);
    }

    #[test]
    fn test_server_id_invalid_parse() {
        let result = "not-a-number".parse::<i64>();
        assert!(result.is_err());
    }

    #[test]
    fn test_make_configuration() {
        let config = make_configuration("test-key");
        assert_eq!(config.bearer_access_token, Some("test-key".to_string()));
    }

    #[test]
    fn test_hetzner_provider_new() {
        let _provider = HetznerProvider::new();
        // Verifies construction succeeds without panic
    }

    // ── Integration tests (require HETZNER_API_KEY env var) ─────────────

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_valid() {
        let api_key = std::env::var("HETZNER_API_KEY")
            .expect("HETZNER_API_KEY must be set for integration tests");
        let provider = HetznerProvider::new();
        let result = provider.validate_credential(&api_key).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_invalid() {
        let provider = HetznerProvider::new();
        let result = provider.validate_credential("invalid-key-12345").await;
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_regions() {
        let api_key = std::env::var("HETZNER_API_KEY")
            .expect("HETZNER_API_KEY must be set for integration tests");
        let provider = HetznerProvider::new();
        let regions = provider.list_regions(&api_key).await.unwrap();

        assert!(!regions.is_empty());
        // Verify sorted by cost ascending
        for window in regions.windows(2) {
            assert!(window[0].hourly_cost <= window[1].hourly_cost);
        }
        // Verify cache was populated
        let cache = provider.region_server_types.read().await;
        assert!(!cache.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_ssh_key_create_delete() {
        let api_key = std::env::var("HETZNER_API_KEY")
            .expect("HETZNER_API_KEY must be set for integration tests");
        let provider = HetznerProvider::new();

        // Use a dummy ed25519 public key for testing
        let public_key =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl test@oh-my-vpn";
        let label = "oh-my-vpn-test-key";

        let key_id = provider
            .create_ssh_key(&api_key, public_key, label)
            .await
            .unwrap();
        assert!(!key_id.is_empty());

        // Verify key_id is a valid i64 string
        assert!(key_id.parse::<i64>().is_ok());

        // Clean up
        provider.delete_ssh_key(&api_key, &key_id).await.unwrap();
    }
}
