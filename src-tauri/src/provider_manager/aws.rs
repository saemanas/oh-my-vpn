//! AWS EC2 cloud provider implementation.
//!
//! Implements `CloudProvider` for AWS using `aws-sdk-ec2` and `aws-sdk-pricing`.
//! See ADR-0002 (Rust SDK) and ADR-0005 (Provider Pricing API).
//!
//! ## AWS-specific design decisions
//!
//! - **Region-scoped operations**: AWS EC2 is region-scoped, so server_id and key_id
//!   encode the region as compound IDs (e.g., `us-east-1/i-abc/sg-def`).
//! - **Deferred SSH key import**: `create_ssh_key` caches key material internally
//!   because the trait method doesn't receive a region. Actual `import_key_pair`
//!   happens in `create_server` where the target region is known.
//! - **Credentials format**: Single `api_key` stores `ACCESS_KEY_ID:SECRET_ACCESS_KEY`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use aws_config::Region;
use aws_sdk_ec2::types::{
    Filter, InstanceStateName, IpPermission, IpRange, ResourceType, Tag, TagSpecification,
};
use base64::Engine;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::error::ProviderError;
use crate::types::{RegionInfo, ServerInfo, ServerStatus};

use super::CloudProvider;

// ── Internal types ──────────────────────────────────────────────────────────

/// Cached SSH key material awaiting import during `create_server`.
struct PendingSshKey {
    label: String,
    public_key: String,
}

/// AWS EC2 cloud provider.
///
/// Holds internal caches for region-to-instance-type mapping, pending SSH key
/// material, and AMI IDs per region.
pub struct AwsProvider {
    /// Region → instance type cache, populated by `list_regions`.
    region_instance_types: Arc<RwLock<HashMap<String, String>>>,
    /// Pending SSH key awaiting region-scoped import in `create_server`.
    pending_ssh_key: Arc<RwLock<Option<PendingSshKey>>>,
    /// Region → Ubuntu AMI ID cache.
    ami_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl AwsProvider {
    /// Create a new AwsProvider with empty caches.
    pub fn new() -> Self {
        Self {
            region_instance_types: Arc::new(RwLock::new(HashMap::new())),
            pending_ssh_key: Arc::new(RwLock::new(None)),
            ami_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Resolve the latest Ubuntu 24.04 AMI in the given region.
    ///
    /// Checks internal cache first; on miss queries EC2 `describe_images`
    /// with Canonical's owner ID and caches the result.
    async fn resolve_ami(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        region: &str,
    ) -> Result<String, ProviderError> {
        // Check cache
        {
            let cache = self.ami_cache.read().await;
            if let Some(ami_id) = cache.get(region) {
                return Ok(ami_id.clone());
            }
        }

        // Query Canonical's Ubuntu 24.04 (Noble) AMIs
        let response = ec2_client
            .describe_images()
            .owners("099720109477") // Canonical's AWS account
            .filters(
                Filter::builder()
                    .name("name")
                    .values("ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*")
                    .build(),
            )
            .filters(
                Filter::builder()
                    .name("state")
                    .values("available")
                    .build(),
            )
            .send()
            .await
            .map_err(map_aws_error)?;

        // Sort by creation_date descending, take latest
        let mut images: Vec<_> = response.images().to_vec();
        images.sort_by(|a, b| {
            b.creation_date()
                .unwrap_or_default()
                .cmp(&a.creation_date().unwrap_or_default())
        });

        let ami_id = images
            .first()
            .and_then(|image| image.image_id())
            .ok_or_else(|| {
                ProviderError::ProvisioningFailed(format!(
                    "No Ubuntu 24.04 AMI found in region {}",
                    region
                ))
            })?
            .to_string();

        // Cache result
        {
            let mut cache = self.ami_cache.write().await;
            cache.insert(region.to_string(), ami_id.clone());
        }

        Ok(ami_id)
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Parse `"ACCESS_KEY_ID:SECRET_ACCESS_KEY"` into its two components.
fn parse_aws_credentials(api_key: &str) -> Result<(&str, &str), ProviderError> {
    let (access_key, secret_key) = api_key.split_once(':').ok_or_else(|| {
        ProviderError::AuthInvalidKey(
            "AWS credentials must be in ACCESS_KEY_ID:SECRET_ACCESS_KEY format".to_string(),
        )
    })?;

    if access_key.is_empty() || secret_key.is_empty() {
        return Err(ProviderError::AuthInvalidKey(
            "AWS access key ID and secret access key must not be empty".to_string(),
        ));
    }

    Ok((access_key, secret_key))
}

/// Build a region-scoped AWS SDK config from the compound API key.
async fn make_sdk_config(
    api_key: &str,
    region: &str,
) -> Result<aws_config::SdkConfig, ProviderError> {
    let (access_key, secret_key) = parse_aws_credentials(api_key)?;
    let credentials = aws_sdk_ec2::config::Credentials::new(
        access_key,
        secret_key,
        None,
        None,
        "oh-my-vpn",
    );
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .credentials_provider(credentials)
        .load()
        .await;
    Ok(config)
}

/// Map any AWS SDK error (Debug-printable) to a `ProviderError`.
///
/// Uses pattern matching on the debug string to identify common AWS error codes.
/// This avoids generic type parameter issues with the AWS SDK's `SdkError<E, R>`.
fn map_aws_error<T: std::fmt::Debug>(error: T) -> ProviderError {
    let debug_string = format!("{:?}", error);

    // Check for timeout patterns
    if debug_string.contains("TimeoutError") || debug_string.contains("timed out") {
        return ProviderError::Timeout;
    }

    // Check for authentication errors
    if debug_string.contains("AuthFailure")
        || debug_string.contains("InvalidClientTokenId")
        || debug_string.contains("SignatureDoesNotMatch")
    {
        return ProviderError::AuthInvalidKey(format!(
            "AWS authentication failed: {}",
            debug_string
        ));
    }

    // Check for permission errors
    if debug_string.contains("UnauthorizedOperation")
        || debug_string.contains("AccessDenied")
        || debug_string.contains("403")
    {
        return ProviderError::AuthInsufficientPermissions(format!(
            "AWS insufficient permissions: {}",
            debug_string
        ));
    }

    // Check for not found
    if debug_string.contains("NotFound") || debug_string.contains("404") {
        return ProviderError::NotFound(format!(
            "AWS resource not found: {}",
            debug_string
        ));
    }

    // Check for rate limiting
    if debug_string.contains("Throttling") || debug_string.contains("429") {
        return ProviderError::RateLimited {
            retry_after_seconds: 60,
        };
    }

    // Check for server errors
    if debug_string.contains("500")
        || debug_string.contains("502")
        || debug_string.contains("503")
        || debug_string.contains("InternalError")
    {
        return ProviderError::ServerError(format!("AWS server error: {}", debug_string));
    }

    ProviderError::Other(anyhow::anyhow!("AWS API error: {}", debug_string))
}

/// Convert an AWS instance state to our `ServerStatus` enum.
fn map_instance_status(state: Option<&aws_sdk_ec2::types::InstanceState>) -> ServerStatus {
    match state.and_then(|s| s.name()) {
        Some(InstanceStateName::Running) => ServerStatus::Running,
        Some(InstanceStateName::Pending) => ServerStatus::Provisioning,
        Some(InstanceStateName::ShuttingDown) | Some(InstanceStateName::Terminated) => {
            ServerStatus::Deleting
        }
        _ => ServerStatus::Provisioning,
    }
}

/// Parse compound server_id: `"{region}/{instance_id}/{security_group_id}"`.
fn parse_compound_server_id(server_id: &str) -> Result<(&str, &str, &str), ProviderError> {
    let parts: Vec<&str> = server_id.splitn(3, '/').collect();
    if parts.len() != 3 {
        return Err(ProviderError::Other(anyhow::anyhow!(
            "Invalid AWS server ID format '{}' -- expected 'region/instance_id/sg_id'",
            server_id
        )));
    }
    Ok((parts[0], parts[1], parts[2]))
}

/// Parse compound key_id: `"{region}/{key_name}"` or `"pending/{label}"`.
fn parse_compound_key_id(key_id: &str) -> Result<(&str, &str), ProviderError> {
    let (first, second) = key_id.split_once('/').ok_or_else(|| {
        ProviderError::Other(anyhow::anyhow!(
            "Invalid AWS key ID format '{}' -- expected 'region/key_name' or 'pending/label'",
            key_id
        ))
    })?;
    Ok((first, second))
}

/// Parse a single pricing entry JSON from the AWS Pricing API `GetProducts` response.
///
/// Returns `(region_code, hourly_cost)` or `None` if parsing fails or price is zero.
fn parse_pricing_entry(json_string: &str) -> Option<(String, f64)> {
    let value: serde_json::Value = serde_json::from_str(json_string).ok()?;

    // Extract region code from product attributes
    let region_code = value
        .pointer("/product/attributes/regionCode")?
        .as_str()?
        .to_string();

    // Navigate: terms.OnDemand.{sku_id}.priceDimensions.{rate_code}.pricePerUnit.USD
    let on_demand = value.pointer("/terms/OnDemand")?;
    let sku_entry = on_demand.as_object()?.values().next()?;
    let price_dimensions = sku_entry.get("priceDimensions")?;
    let rate_entry = price_dimensions.as_object()?.values().next()?;
    let usd_price = rate_entry.pointer("/pricePerUnit/USD")?.as_str()?;

    let hourly_cost = usd_price.parse::<f64>().ok()?;

    // Skip $0.00 entries (reserved or spot placeholders)
    if hourly_cost <= 0.0 {
        return None;
    }

    Some((region_code, hourly_cost))
}

/// Map an AWS region code to a human-readable display name.
fn get_region_display_name(region_code: &str) -> String {
    match region_code {
        "us-east-1" => "N. Virginia, US".to_string(),
        "us-east-2" => "Ohio, US".to_string(),
        "us-west-1" => "N. California, US".to_string(),
        "us-west-2" => "Oregon, US".to_string(),
        "eu-west-1" => "Ireland, EU".to_string(),
        "eu-west-2" => "London, UK".to_string(),
        "eu-west-3" => "Paris, FR".to_string(),
        "eu-central-1" => "Frankfurt, DE".to_string(),
        "eu-central-2" => "Zurich, CH".to_string(),
        "eu-north-1" => "Stockholm, SE".to_string(),
        "eu-south-1" => "Milan, IT".to_string(),
        "ap-southeast-1" => "Singapore, SG".to_string(),
        "ap-southeast-2" => "Sydney, AU".to_string(),
        "ap-northeast-1" => "Tokyo, JP".to_string(),
        "ap-northeast-2" => "Seoul, KR".to_string(),
        "ap-northeast-3" => "Osaka, JP".to_string(),
        "ap-south-1" => "Mumbai, IN".to_string(),
        "ap-east-1" => "Hong Kong, HK".to_string(),
        "sa-east-1" => "Sao Paulo, BR".to_string(),
        "ca-central-1" => "Montreal, CA".to_string(),
        "me-south-1" => "Bahrain, ME".to_string(),
        "me-central-1" => "UAE, ME".to_string(),
        "af-south-1" => "Cape Town, ZA".to_string(),
        "il-central-1" => "Tel Aviv, IL".to_string(),
        _ => region_code.to_string(),
    }
}

// ── CloudProvider implementation ────────────────────────────────────────────

#[async_trait]
impl CloudProvider for AwsProvider {
    async fn validate_credential(&self, api_key: &str) -> Result<(), ProviderError> {
        let config = make_sdk_config(api_key, "us-east-1").await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        ec2_client
            .describe_instances()
            .max_results(5)
            .send()
            .await
            .map_err(map_aws_error)?;

        Ok(())
    }

    async fn list_regions(&self, api_key: &str) -> Result<Vec<RegionInfo>, ProviderError> {
        let config = make_sdk_config(api_key, "us-east-1").await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        // 1. Get available (opted-in) regions
        let regions_response = ec2_client
            .describe_regions()
            .all_regions(false)
            .send()
            .await
            .map_err(map_aws_error)?;

        let available_regions: Vec<String> = regions_response
            .regions()
            .iter()
            .filter_map(|r| r.region_name().map(|s| s.to_string()))
            .collect();

        // 2. Get t3.nano pricing across all regions via Pricing API
        let pricing_client = aws_sdk_pricing::Client::new(&config);
        let mut pricing_map: HashMap<String, f64> = HashMap::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = pricing_client
                .get_products()
                .service_code("AmazonEC2")
                .filters(
                    aws_sdk_pricing::types::Filter::builder()
                        .field("instanceType")
                        .value("t3.nano")
                        .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                        .build()
                        .map_err(|e| {
                            ProviderError::Other(anyhow::anyhow!("Filter build error: {}", e))
                        })?,
                )
                .filters(
                    aws_sdk_pricing::types::Filter::builder()
                        .field("operatingSystem")
                        .value("Linux")
                        .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                        .build()
                        .map_err(|e| {
                            ProviderError::Other(anyhow::anyhow!("Filter build error: {}", e))
                        })?,
                )
                .filters(
                    aws_sdk_pricing::types::Filter::builder()
                        .field("preInstalledSw")
                        .value("NA")
                        .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                        .build()
                        .map_err(|e| {
                            ProviderError::Other(anyhow::anyhow!("Filter build error: {}", e))
                        })?,
                )
                .filters(
                    aws_sdk_pricing::types::Filter::builder()
                        .field("tenancy")
                        .value("Shared")
                        .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                        .build()
                        .map_err(|e| {
                            ProviderError::Other(anyhow::anyhow!("Filter build error: {}", e))
                        })?,
                )
                .filters(
                    aws_sdk_pricing::types::Filter::builder()
                        .field("capacitystatus")
                        .value("Used")
                        .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                        .build()
                        .map_err(|e| {
                            ProviderError::Other(anyhow::anyhow!("Filter build error: {}", e))
                        })?,
                );

            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            let pricing_response = request.send().await.map_err(map_aws_error)?;

            for price_json in pricing_response.price_list() {
                if let Some((region_code, hourly_cost)) = parse_pricing_entry(price_json) {
                    // Keep the lowest price per region (skip duplicates)
                    let entry = pricing_map.entry(region_code).or_insert(f64::MAX);
                    if hourly_cost < *entry {
                        *entry = hourly_cost;
                    }
                }
            }

            next_token = pricing_response.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }

        // 3. Build RegionInfo list and populate cache
        let mut regions: Vec<RegionInfo> = Vec::new();
        let mut cache_update: HashMap<String, String> = HashMap::new();

        for region_code in &available_regions {
            if let Some(&hourly_cost) = pricing_map.get(region_code) {
                regions.push(RegionInfo {
                    region: region_code.clone(),
                    display_name: get_region_display_name(region_code),
                    instance_type: "t3.nano".to_string(),
                    hourly_cost,
                });
                cache_update.insert(region_code.clone(), "t3.nano".to_string());
            }
        }

        // Sort by hourly_cost ascending
        regions.sort_by(|a, b| {
            a.hourly_cost
                .partial_cmp(&b.hourly_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Populate cache
        {
            let mut cache = self.region_instance_types.write().await;
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
        // Validate credentials format (fail fast)
        let _ = parse_aws_credentials(api_key)?;

        // Cache key material for deferred import in create_server
        let mut pending = self.pending_ssh_key.write().await;
        *pending = Some(PendingSshKey {
            label: label.to_string(),
            public_key: public_key.to_string(),
        });

        // Return synthetic key_id -- actual import happens in create_server
        Ok(format!("pending/{}", label))
    }

    async fn delete_ssh_key(
        &self,
        api_key: &str,
        key_id: &str,
    ) -> Result<(), ProviderError> {
        if key_id.starts_with("pending/") {
            // Key was never imported -- just clear internal cache
            let mut pending = self.pending_ssh_key.write().await;
            *pending = None;
            return Ok(());
        }

        // Parse compound key_id: "{region}/{key_name}"
        let (region, key_name) = parse_compound_key_id(key_id)?;
        let config = make_sdk_config(api_key, region).await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        ec2_client
            .delete_key_pair()
            .key_name(key_name)
            .send()
            .await
            .map_err(map_aws_error)?;

        Ok(())
    }

    async fn create_server(
        &self,
        api_key: &str,
        region: &str,
        ssh_key_id: &str,
        cloud_init: &str,
    ) -> Result<ServerInfo, ProviderError> {
        let config = make_sdk_config(api_key, region).await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // ── Step 1: Import SSH key in target region ─────────────────────

        let key_label = ssh_key_id.strip_prefix("pending/").ok_or_else(|| {
            ProviderError::Other(anyhow::anyhow!(
                "Expected pending SSH key ID, got '{}'",
                ssh_key_id
            ))
        })?;

        let public_key_material = {
            let pending = self.pending_ssh_key.read().await;
            let ssh_key = pending.as_ref().ok_or_else(|| {
                ProviderError::Other(anyhow::anyhow!("No pending SSH key found"))
            })?;
            ssh_key.public_key.clone()
        };

        ec2_client
            .import_key_pair()
            .key_name(key_label)
            .public_key_material(aws_sdk_ec2::primitives::Blob::new(
                public_key_material.as_bytes(),
            ))
            .send()
            .await
            .map_err(map_aws_error)?;

        // Clear pending cache after successful import
        {
            let mut pending = self.pending_ssh_key.write().await;
            *pending = None;
        }

        // From here on, we need to clean up the key on failure
        let imported_key_name = key_label.to_string();

        // ── Step 2: Resolve Ubuntu AMI ──────────────────────────────────

        let ami_id = match self.resolve_ami(&ec2_client, region).await {
            Ok(id) => id,
            Err(error) => {
                // Cleanup: delete imported key
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;
                return Err(error);
            }
        };

        // ── Step 3: Find default VPC ────────────────────────────────────

        let vpc_id = match ec2_client
            .describe_vpcs()
            .filters(
                Filter::builder()
                    .name("isDefault")
                    .values("true")
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response
                .vpcs()
                .first()
                .and_then(|vpc| vpc.vpc_id())
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    ProviderError::ProvisioningFailed(
                        "No default VPC found. Create a default VPC in the AWS Console."
                            .to_string(),
                    )
                }),
            Err(error) => {
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;
                return Err(map_aws_error(error));
            }
        };

        let vpc_id = match vpc_id {
            Ok(id) => id,
            Err(error) => {
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;
                return Err(error);
            }
        };

        // ── Step 4: Create security group (WireGuard UDP only) ──────────

        let sg_name = format!("oh-my-vpn-{}", timestamp);

        let sg_response = match ec2_client
            .create_security_group()
            .group_name(&sg_name)
            .description("Oh My VPN -- WireGuard UDP only (ephemeral)")
            .vpc_id(&vpc_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;
                return Err(map_aws_error(error));
            }
        };

        let security_group_id = sg_response
            .group_id()
            .ok_or_else(|| {
                ProviderError::ProvisioningFailed(
                    "AWS did not return security group ID".to_string(),
                )
            })?
            .to_string();

        // Allow inbound WireGuard UDP 51820
        if let Err(error) = ec2_client
            .authorize_security_group_ingress()
            .group_id(&security_group_id)
            .ip_permissions(
                IpPermission::builder()
                    .ip_protocol("udp")
                    .from_port(51820)
                    .to_port(51820)
                    .ip_ranges(
                        IpRange::builder()
                            .cidr_ip("0.0.0.0/0")
                            .description("WireGuard UDP")
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await
        {
            // Cleanup: delete SG + key
            let _ = ec2_client
                .delete_security_group()
                .group_id(&security_group_id)
                .send()
                .await;
            let _ = ec2_client
                .delete_key_pair()
                .key_name(&imported_key_name)
                .send()
                .await;
            return Err(map_aws_error(error));
        }

        // ── Step 5: Launch EC2 instance ─────────────────────────────────

        let user_data_encoded =
            base64::engine::general_purpose::STANDARD.encode(cloud_init.as_bytes());

        let tag_spec = TagSpecification::builder()
            .resource_type(ResourceType::Instance)
            .tags(
                Tag::builder()
                    .key("Name")
                    .value(format!("oh-my-vpn-{}", timestamp))
                    .build(),
            )
            .build();

        let run_response = match ec2_client
            .run_instances()
            .image_id(&ami_id)
            .instance_type(aws_sdk_ec2::types::InstanceType::T3Nano)
            .min_count(1)
            .max_count(1)
            .key_name(&imported_key_name)
            .security_group_ids(&security_group_id)
            .user_data(&user_data_encoded)
            .tag_specifications(tag_spec)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                // Cleanup: delete SG + key
                let _ = ec2_client
                    .delete_security_group()
                    .group_id(&security_group_id)
                    .send()
                    .await;
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;
                return Err(map_aws_error(error));
            }
        };

        let instance_id = run_response
            .instances()
            .first()
            .and_then(|i| i.instance_id())
            .ok_or_else(|| {
                ProviderError::ProvisioningFailed(
                    "AWS did not return instance ID".to_string(),
                )
            })?
            .to_string();

        // ── Step 6: Poll until running ──────────────────────────────────

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
                // Cleanup: terminate instance + delete SG + delete key
                let _ = ec2_client
                    .terminate_instances()
                    .instance_ids(&instance_id)
                    .send()
                    .await;
                sleep(Duration::from_secs(5)).await;
                let _ = ec2_client
                    .delete_security_group()
                    .group_id(&security_group_id)
                    .send()
                    .await;
                let _ = ec2_client
                    .delete_key_pair()
                    .key_name(&imported_key_name)
                    .send()
                    .await;

                return Err(ProviderError::ProvisioningFailed(format!(
                    "Server {} did not reach Running status within {}s",
                    instance_id,
                    max_wait.as_secs()
                )));
            }

            let describe_response = ec2_client
                .describe_instances()
                .instance_ids(&instance_id)
                .send()
                .await;

            if let Ok(output) = describe_response {
                if let Some(instance) = output
                    .reservations()
                    .first()
                    .and_then(|r| r.instances().first())
                {
                    let status = map_instance_status(instance.state());
                    if status == ServerStatus::Running {
                        let public_ip = instance
                            .public_ip_address()
                            .unwrap_or_default()
                            .to_string();

                        return Ok(ServerInfo {
                            server_id: format!(
                                "{}/{}/{}",
                                region, instance_id, security_group_id
                            ),
                            public_ip,
                            status: ServerStatus::Running,
                        });
                    }
                }
            }
        }
    }

    async fn destroy_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<(), ProviderError> {
        let (region, instance_id, security_group_id) = parse_compound_server_id(server_id)?;
        let config = make_sdk_config(api_key, region).await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        // Terminate instance
        ec2_client
            .terminate_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .map_err(map_aws_error)?;

        // Wait briefly before attempting SG deletion
        sleep(Duration::from_secs(5)).await;

        // Delete security group (best-effort with retries)
        // SG deletion fails while instance is still associated.
        for attempt in 0..3u64 {
            match ec2_client
                .delete_security_group()
                .group_id(security_group_id)
                .send()
                .await
            {
                Ok(_) => break,
                Err(_) if attempt < 2 => {
                    sleep(Duration::from_secs(5 * (attempt + 1))).await;
                }
                Err(_) => {
                    // Best-effort: orphaned SGs named "oh-my-vpn-{timestamp}"
                    // can be identified and cleaned up manually.
                    break;
                }
            }
        }

        Ok(())
    }

    async fn get_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<Option<ServerInfo>, ProviderError> {
        let (region, instance_id, _security_group_id) = parse_compound_server_id(server_id)?;
        let config = make_sdk_config(api_key, region).await?;
        let ec2_client = aws_sdk_ec2::Client::new(&config);

        let response = ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await;

        match response {
            Ok(output) => {
                let instance = output
                    .reservations()
                    .first()
                    .and_then(|r| r.instances().first());

                match instance {
                    Some(inst) => {
                        // AWS keeps terminated instances visible for ~1 hour.
                        // Treat Terminated as None for orphan detection correctness.
                        if let Some(state) = inst.state() {
                            if state.name() == Some(&InstanceStateName::Terminated) {
                                return Ok(None);
                            }
                        }

                        let status = map_instance_status(inst.state());
                        let public_ip = inst
                            .public_ip_address()
                            .unwrap_or_default()
                            .to_string();

                        Ok(Some(ServerInfo {
                            server_id: server_id.to_string(),
                            public_ip,
                            status,
                        }))
                    }
                    None => Ok(None),
                }
            }
            Err(error) => {
                // InvalidInstanceID.NotFound → return None (not an error)
                let error_string = format!("{:?}", error);
                if error_string.contains("InvalidInstanceID") {
                    return Ok(None);
                }
                Err(map_aws_error(error))
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
    fn test_parse_aws_credentials_valid() {
        let (access, secret) = parse_aws_credentials("AKIAIOSFODNN7EXAMPLE:wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").unwrap();
        assert_eq!(access, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(secret, "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    }

    #[test]
    fn test_parse_aws_credentials_missing_colon() {
        let result = parse_aws_credentials("no-colon-here");
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[test]
    fn test_parse_aws_credentials_empty_access_key() {
        let result = parse_aws_credentials(":secret");
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[test]
    fn test_parse_aws_credentials_empty_secret_key() {
        let result = parse_aws_credentials("access:");
        assert!(matches!(result, Err(ProviderError::AuthInvalidKey(_))));
    }

    #[test]
    fn test_map_instance_status_running() {
        let state = aws_sdk_ec2::types::InstanceState::builder()
            .name(InstanceStateName::Running)
            .build();
        assert_eq!(map_instance_status(Some(&state)), ServerStatus::Running);
    }

    #[test]
    fn test_map_instance_status_pending() {
        let state = aws_sdk_ec2::types::InstanceState::builder()
            .name(InstanceStateName::Pending)
            .build();
        assert_eq!(map_instance_status(Some(&state)), ServerStatus::Provisioning);
    }

    #[test]
    fn test_map_instance_status_terminated() {
        let state = aws_sdk_ec2::types::InstanceState::builder()
            .name(InstanceStateName::Terminated)
            .build();
        assert_eq!(map_instance_status(Some(&state)), ServerStatus::Deleting);
    }

    #[test]
    fn test_map_instance_status_shutting_down() {
        let state = aws_sdk_ec2::types::InstanceState::builder()
            .name(InstanceStateName::ShuttingDown)
            .build();
        assert_eq!(map_instance_status(Some(&state)), ServerStatus::Deleting);
    }

    #[test]
    fn test_map_instance_status_none() {
        assert_eq!(map_instance_status(None), ServerStatus::Provisioning);
    }

    #[test]
    fn test_parse_compound_server_id_valid() {
        let (region, instance, sg) =
            parse_compound_server_id("us-east-1/i-0abc123def456/sg-0abc123def456").unwrap();
        assert_eq!(region, "us-east-1");
        assert_eq!(instance, "i-0abc123def456");
        assert_eq!(sg, "sg-0abc123def456");
    }

    #[test]
    fn test_parse_compound_server_id_invalid_too_few() {
        let result = parse_compound_server_id("us-east-1/i-abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_compound_server_id_invalid_no_slash() {
        let result = parse_compound_server_id("no-slashes");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_compound_key_id_region_scoped() {
        let (region, key_name) = parse_compound_key_id("us-east-1/oh-my-vpn-12345").unwrap();
        assert_eq!(region, "us-east-1");
        assert_eq!(key_name, "oh-my-vpn-12345");
    }

    #[test]
    fn test_parse_compound_key_id_pending() {
        let (prefix, label) = parse_compound_key_id("pending/oh-my-vpn-12345").unwrap();
        assert_eq!(prefix, "pending");
        assert_eq!(label, "oh-my-vpn-12345");
    }

    #[test]
    fn test_parse_compound_key_id_invalid() {
        let result = parse_compound_key_id("no-slash");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_region_display_name_known() {
        assert_eq!(get_region_display_name("us-east-1"), "N. Virginia, US");
        assert_eq!(get_region_display_name("eu-central-1"), "Frankfurt, DE");
        assert_eq!(get_region_display_name("ap-northeast-1"), "Tokyo, JP");
    }

    #[test]
    fn test_get_region_display_name_unknown() {
        assert_eq!(
            get_region_display_name("xx-unknown-99"),
            "xx-unknown-99"
        );
    }

    #[test]
    fn test_parse_pricing_entry_valid() {
        let json = r#"{
            "product": {
                "attributes": {
                    "regionCode": "us-east-1",
                    "instanceType": "t3.nano"
                }
            },
            "terms": {
                "OnDemand": {
                    "SKU123.JRTCKXETXF": {
                        "priceDimensions": {
                            "SKU123.JRTCKXETXF.6YS6EN2CT7": {
                                "pricePerUnit": {
                                    "USD": "0.0052000000"
                                }
                            }
                        }
                    }
                }
            }
        }"#;

        let result = parse_pricing_entry(json);
        assert!(result.is_some());
        let (region, cost) = result.unwrap();
        assert_eq!(region, "us-east-1");
        assert!((cost - 0.0052).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_pricing_entry_zero_price() {
        let json = r#"{
            "product": {
                "attributes": {
                    "regionCode": "us-east-1"
                }
            },
            "terms": {
                "OnDemand": {
                    "SKU123.JRTCKXETXF": {
                        "priceDimensions": {
                            "SKU123.JRTCKXETXF.6YS6EN2CT7": {
                                "pricePerUnit": {
                                    "USD": "0.0000000000"
                                }
                            }
                        }
                    }
                }
            }
        }"#;

        assert!(parse_pricing_entry(json).is_none());
    }

    #[test]
    fn test_parse_pricing_entry_malformed() {
        assert!(parse_pricing_entry("not json").is_none());
        assert!(parse_pricing_entry("{}").is_none());
        assert!(parse_pricing_entry(r#"{"product": {}}"#).is_none());
    }

    #[test]
    fn test_aws_provider_new() {
        let _provider = AwsProvider::new();
        // Verifies construction succeeds without panic
    }

    #[tokio::test]
    async fn test_create_ssh_key_returns_pending_id() {
        let provider = AwsProvider::new();
        // Use a valid format credential for the parse check
        let key_id = provider
            .create_ssh_key(
                "AKIATEST:SECRETTEST",
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
        let provider = AwsProvider::new();
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
            .delete_ssh_key("AKIATEST:SECRETTEST", "pending/test")
            .await
            .unwrap();

        let pending = provider.pending_ssh_key.read().await;
        assert!(pending.is_none());
    }

    // ── Integration tests (require AWS_TEST_CREDENTIALS env var) ────────

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_valid() {
        let api_key = std::env::var("AWS_TEST_CREDENTIALS")
            .expect("AWS_TEST_CREDENTIALS must be set (ACCESS_KEY:SECRET_KEY)");
        let provider = AwsProvider::new();
        let result = provider.validate_credential(&api_key).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_validate_credential_invalid() {
        let provider = AwsProvider::new();
        let result = provider
            .validate_credential("AKIAINVALID:INVALIDSECRETSECRETSE")
            .await;
        assert!(matches!(
            result,
            Err(ProviderError::AuthInvalidKey(_))
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_regions() {
        let api_key = std::env::var("AWS_TEST_CREDENTIALS")
            .expect("AWS_TEST_CREDENTIALS must be set (ACCESS_KEY:SECRET_KEY)");
        let provider = AwsProvider::new();
        let regions = provider.list_regions(&api_key).await.unwrap();

        assert!(!regions.is_empty());
        // Verify sorted by cost ascending
        for window in regions.windows(2) {
            assert!(window[0].hourly_cost <= window[1].hourly_cost);
        }
        // Verify all instance types are t3.nano
        for region in &regions {
            assert_eq!(region.instance_type, "t3.nano");
        }
        // Verify cache was populated
        let cache = provider.region_instance_types.read().await;
        assert!(!cache.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_server_create_destroy() {
        let api_key = std::env::var("AWS_TEST_CREDENTIALS")
            .expect("AWS_TEST_CREDENTIALS must be set (ACCESS_KEY:SECRET_KEY)");
        let provider = AwsProvider::new();

        // Populate region cache
        let _ = provider.list_regions(&api_key).await.unwrap();

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
            .create_server(&api_key, "us-east-1", &key_id, cloud_init)
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
        sleep(Duration::from_secs(10)).await;
        let check = provider
            .get_server(&api_key, &server_info.server_id)
            .await
            .unwrap();
        assert!(check.is_none());

        // Clean up key (should already be imported, so use region-scoped ID)
        let _ = provider
            .delete_ssh_key(
                &api_key,
                &format!("us-east-1/oh-my-vpn-test-key"),
            )
            .await;
    }
}
