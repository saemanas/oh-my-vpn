//! Shared type definitions used across the crate.
//!
//! Centralises cross-cutting value types so every module imports from one
//! canonical location rather than re-declaring the same shapes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported cloud providers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Hetzner,
    Aws,
    Gcp,
}

impl Provider {
    /// Returns the macOS Keychain service name used to store credentials for
    /// this provider (e.g. `"oh-my-vpn.hetzner"`).
    pub fn service_name(&self) -> String {
        match self {
            Provider::Hetzner => "oh-my-vpn.hetzner".to_string(),
            Provider::Aws => "oh-my-vpn.aws".to_string(),
            Provider::Gcp => "oh-my-vpn.gcp".to_string(),
        }
    }

    /// Parses a Keychain service name back into a `Provider`.
    ///
    /// Returns `None` if the name does not correspond to any known provider.
    pub fn from_service_name(name: &str) -> Option<Provider> {
        match name {
            "oh-my-vpn.hetzner" => Some(Provider::Hetzner),
            "oh-my-vpn.aws" => Some(Provider::Aws),
            "oh-my-vpn.gcp" => Some(Provider::Gcp),
            _ => None,
        }
    }
}

/// Cloud region with pricing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo {
    /// Cloud region code (e.g., "fsn1", "us-east-1").
    pub region: String,
    /// Human-readable name (e.g., "Falkenstein, DE").
    pub display_name: String,
    /// Cheapest instance type name.
    pub instance_type: String,
    /// USD per hour.
    pub hourly_cost: f64,
}

/// Server information returned by cloud provider operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Provider-side server ID.
    pub server_id: String,
    /// Public IP address of the server.
    pub public_ip: String,
    /// Current server status.
    pub status: ServerStatus,
}

/// Server lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Provisioning,
    Running,
    Deleting,
}

/// Provider registration and validation status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderStatus {
    Valid,
    Invalid,
    Unchecked,
}

/// Provider information for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Which cloud provider.
    pub provider: Provider,
    /// Credential validation status.
    pub status: ProviderStatus,
    /// Human-readable identifier from Keychain account field.
    pub account_label: String,
}

/// Action to take on an orphaned server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrphanAction {
    Destroy,
    Reconnect,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provider::Hetzner => write!(f, "Hetzner"),
            Provider::Aws => write!(f, "AWS"),
            Provider::Gcp => write!(f, "GCP"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name() {
        assert_eq!(Provider::Hetzner.service_name(), "oh-my-vpn.hetzner");
        assert_eq!(Provider::Aws.service_name(), "oh-my-vpn.aws");
        assert_eq!(Provider::Gcp.service_name(), "oh-my-vpn.gcp");
    }

    #[test]
    fn test_from_service_name() {
        assert_eq!(Provider::from_service_name("oh-my-vpn.hetzner"), Some(Provider::Hetzner));
        assert_eq!(Provider::from_service_name("oh-my-vpn.aws"), Some(Provider::Aws));
        assert_eq!(Provider::from_service_name("oh-my-vpn.gcp"), Some(Provider::Gcp));
        assert_eq!(Provider::from_service_name("oh-my-vpn.unknown"), None);
        assert_eq!(Provider::from_service_name(""), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(Provider::Hetzner.to_string(), "Hetzner");
        assert_eq!(Provider::Aws.to_string(), "AWS");
        assert_eq!(Provider::Gcp.to_string(), "GCP");
    }

    #[test]
    fn test_serde_lowercase() {
        let json = serde_json::to_string(&Provider::Hetzner).unwrap();
        assert_eq!(json, r#""hetzner""#);

        let json = serde_json::to_string(&Provider::Aws).unwrap();
        assert_eq!(json, r#""aws""#);

        let json = serde_json::to_string(&Provider::Gcp).unwrap();
        assert_eq!(json, r#""gcp""#);

        let p: Provider = serde_json::from_str(r#""hetzner""#).unwrap();
        assert_eq!(p, Provider::Hetzner);
    }
}
