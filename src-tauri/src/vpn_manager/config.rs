//! WireGuard configuration file management.
//!
//! Generates wg-quick compatible INI config files, writes them to disk with
//! restricted permissions (0o600), and removes them after tunnel teardown.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::error::VpnError;

/// Path where the WireGuard configuration file is written on disk.
pub const CONFIG_PATH: &str = "/tmp/oh-my-vpn-wg0.conf";

/// A wg-quick compatible WireGuard tunnel configuration.
///
/// All fields are plain strings so callers can supply pre-encoded values
/// (Base64 keys, CIDR addresses, host:port endpoints) without conversion.
#[derive(Debug)]
pub struct WireGuardConfig {
    /// Base64-encoded Curve25519 private key for the local interface.
    pub interface_private_key: String,
    /// CIDR address assigned to the local WireGuard interface (e.g. `10.0.0.2/32`).
    pub interface_address: String,
    /// DNS server(s) the tunnel should use (e.g. `1.1.1.1`).
    pub interface_dns: String,
    /// Base64-encoded Curve25519 public key of the remote peer.
    pub peer_public_key: String,
    /// `host:port` endpoint of the remote WireGuard peer.
    pub peer_endpoint: String,
    /// Comma-separated CIDR ranges routed through the tunnel (e.g. `0.0.0.0/0`).
    pub peer_allowed_ips: String,
}

impl WireGuardConfig {
    /// Render the configuration as a wg-quick compatible INI string.
    pub fn to_ini(&self) -> String {
        format!(
            "[Interface]\nPrivateKey = {}\nAddress = {}\nDNS = {}\n\n[Peer]\nPublicKey = {}\nEndpoint = {}\nAllowedIPs = {}\n",
            self.interface_private_key,
            self.interface_address,
            self.interface_dns,
            self.peer_public_key,
            self.peer_endpoint,
            self.peer_allowed_ips,
        )
    }

    /// Write the configuration to [`CONFIG_PATH`] with permissions `0o600`.
    ///
    /// Creates the file (or truncates it) and then restricts it to
    /// owner-read/write only so the private key is not world-readable.
    pub fn write(&self) -> Result<(), VpnError> {
        let content = self.to_ini();

        fs::write(CONFIG_PATH, &content)
            .map_err(|e| VpnError::ConfigWriteFailed(format!("Failed to write config: {e}")))?;

        fs::set_permissions(CONFIG_PATH, fs::Permissions::from_mode(0o600)).map_err(|e| {
            VpnError::ConfigPermissionFailed(format!("Failed to set permissions: {e}"))
        })?;

        Ok(())
    }

    /// Remove the configuration file at [`CONFIG_PATH`].
    ///
    /// Idempotent -- returns `Ok(())` if the file does not exist.
    pub fn delete() -> Result<(), VpnError> {
        let path = Path::new(CONFIG_PATH);

        if !path.exists() {
            return Ok(());
        }

        fs::remove_file(path)
            .map_err(|e| VpnError::ConfigDeleteFailed(format!("Failed to delete config: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> WireGuardConfig {
        WireGuardConfig {
            interface_private_key: "aGVsbG8gd29ybGQgdGhpcyBpcyBhIHRlc3Qga2V5".to_string(),
            interface_address: "10.0.0.2/32".to_string(),
            interface_dns: "1.1.1.1".to_string(),
            peer_public_key: "cGVlciBwdWJsaWMga2V5IGZvciBXaXJlR3VhcmQ=".to_string(),
            peer_endpoint: "203.0.113.1:51820".to_string(),
            peer_allowed_ips: "0.0.0.0/0, ::/0".to_string(),
        }
    }

    #[test]
    fn test_to_ini_format() {
        let config = make_config();
        let ini = config.to_ini();

        // Section headers present
        assert!(ini.contains("[Interface]"), "missing [Interface] section");
        assert!(ini.contains("[Peer]"), "missing [Peer] section");

        // All keys present with correct labels
        assert!(ini.contains("PrivateKey = aGVsbG8gd29ybGQgdGhpcyBpcyBhIHRlc3Qga2V5"));
        assert!(ini.contains("Address = 10.0.0.2/32"));
        assert!(ini.contains("DNS = 1.1.1.1"));
        assert!(ini.contains("PublicKey = cGVlciBwdWJsaWMga2V5IGZvciBXaXJlR3VhcmQ="));
        assert!(ini.contains("Endpoint = 203.0.113.1:51820"));
        assert!(ini.contains("AllowedIPs = 0.0.0.0/0, ::/0"));

        // Sections appear in order
        let interface_pos = ini.find("[Interface]").unwrap();
        let peer_pos = ini.find("[Peer]").unwrap();
        assert!(interface_pos < peer_pos, "[Interface] must precede [Peer]");

        // Blank line separates sections
        assert!(ini.contains("\n\n[Peer]"), "blank line required between sections");
    }

    #[test]
    fn test_write_and_delete() {
        // Clean up any leftover file first
        let _ = WireGuardConfig::delete();

        let config = make_config();
        config.write().expect("write should succeed");

        // Verify the file exists and has the expected content
        let content = fs::read_to_string(CONFIG_PATH).expect("file should be readable");
        assert_eq!(content, config.to_ini());

        // Verify permission bits are 0o600
        let metadata = fs::metadata(CONFIG_PATH).expect("metadata should be available");
        let mode = metadata.permissions().mode();
        // Mask to the lower 9 permission bits
        assert_eq!(
            mode & 0o777,
            0o600,
            "file permissions must be 0o600, got {mode:o}"
        );

        // Clean up
        WireGuardConfig::delete().expect("delete should succeed");

        // Verify the file is gone
        assert!(
            !Path::new(CONFIG_PATH).exists(),
            "file should be removed after delete"
        );
    }

    #[test]
    fn test_delete_nonexistent() {
        // Ensure the file does not exist
        let _ = fs::remove_file(CONFIG_PATH);

        // Deleting a non-existent file should return Ok(())
        let result = WireGuardConfig::delete();
        assert!(result.is_ok(), "delete of non-existent file should be Ok");
    }
}
