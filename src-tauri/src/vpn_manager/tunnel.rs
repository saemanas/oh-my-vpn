//! WireGuard tunnel orchestration.
//!
//! Brings up and tears down WireGuard tunnels using wg-quick with the
//! wireguard-go userspace implementation for unprivileged operation.
//!
//! Both `tunnel_up` and `tunnel_down` are `async` and drive subprocess
//! execution via `tokio::process::Command` so they compose naturally with
//! Tauri's async IPC handlers.

use std::path::{Path, PathBuf};

use tokio::process::Command;
use zeroize::Zeroize;

use crate::error::VpnError;
use crate::vpn_manager::config::{WireGuardConfig, CONFIG_PATH};
use crate::vpn_manager::keys::WireGuardKeyPair;

// ── Sidecar Path Resolution ───────────────────────────────────────────────────

/// Resolve the directory that contains the bundled sidecar binaries.
///
/// In both development and release builds the sidecar binaries reside in the
/// same directory as the running executable. Returns a `SidecarNotFound` error
/// if the executable path cannot be determined or has no parent.
pub(crate) fn resolve_sidecar_dir() -> Result<PathBuf, VpnError> {
    let exe_path = std::env::current_exe().map_err(|e| {
        VpnError::SidecarNotFound(format!("Failed to resolve executable path: {e}"))
    })?;

    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            VpnError::SidecarNotFound("Executable path has no parent directory".to_string())
        })
}

/// Validate that all three required sidecar binaries exist in `sidecar_dir`.
///
/// Returns `Ok(())` when all binaries are present. Returns
/// `Err(VpnError::SidecarNotFound)` naming the first missing binary on
/// the first absence encountered.
pub(crate) fn validate_sidecar_binaries(sidecar_dir: &Path) -> Result<(), VpnError> {
    for binary in &["wireguard-go", "wg", "wg-quick"] {
        let path = sidecar_dir.join(binary);
        if !path.exists() {
            return Err(VpnError::SidecarNotFound(format!(
                "Sidecar binary not found: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

// ── osascript Script Builders ─────────────────────────────────────────────────

/// Build the osascript argument that brings the tunnel up with administrator
/// privileges.
///
/// Sets `WG_QUICK_USERSPACE_IMPLEMENTATION` so wg-quick delegates to the
/// bundled wireguard-go userspace implementation instead of the kernel module.
pub(crate) fn build_tunnel_up_script(sidecar_dir: &Path, config_path: &str) -> String {
    let dir = sidecar_dir.display();
    format!(
        "do shell script \
        \"WG_QUICK_USERSPACE_IMPLEMENTATION={dir}/wireguard-go \
        PATH={dir}:$PATH \
        {dir}/wg-quick up {config_path}\" \
        with administrator privileges"
    )
}

/// Build the osascript argument that tears the tunnel down with administrator
/// privileges.
pub(crate) fn build_tunnel_down_script() -> String {
    "do shell script \
    \"wg-quick down oh-my-vpn-wg0\" \
    with administrator privileges"
        .to_string()
}

// ── Public Async API ──────────────────────────────────────────────────────────

/// Bring up a WireGuard VPN tunnel.
///
/// Steps:
/// 1. Generate an ephemeral Curve25519 key pair for this session.
/// 2. Build a `WireGuardConfig` from the supplied parameters and write it to
///    [`CONFIG_PATH`] with `0o600` permissions.
/// 3. Resolve the sidecar binary directory and validate all three binaries.
/// 4. Execute `wg-quick up` via `osascript` with administrator privileges.
/// 5. Delete the config file in both success and failure paths.
///
/// The config file is always deleted before returning so the private key is
/// not left on disk longer than necessary.
pub async fn tunnel_up(
    server_ip: &str,
    server_public_key: &str,
    interface_address: &str,
    dns: &str,
) -> Result<(), VpnError> {
    // 1. Generate ephemeral key pair.
    let key_pair = WireGuardKeyPair::generate();

    // 2. Build config and write to disk.
    let config = WireGuardConfig {
        interface_private_key: key_pair.private_key_base64(),
        interface_address: interface_address.to_string(),
        interface_dns: dns.to_string(),
        peer_public_key: server_public_key.to_string(),
        peer_endpoint: format!("{server_ip}:51820"),
        peer_allowed_ips: "0.0.0.0/0, ::/0".to_string(),
    };

    config.write()?;

    // 3. Validate sidecar binaries. Clean up config before returning on error.
    let sidecar_dir = match resolve_sidecar_dir() {
        Ok(dir) => dir,
        Err(e) => {
            let _ = WireGuardConfig::delete();
            return Err(e);
        }
    };

    if let Err(e) = validate_sidecar_binaries(&sidecar_dir) {
        let _ = WireGuardConfig::delete();
        return Err(e);
    }

    // 4. Execute wg-quick up via osascript.
    let script = build_tunnel_up_script(&sidecar_dir, CONFIG_PATH);

    let output_result = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await;

    // 5. Always delete config (finally pattern) before inspecting the result.
    let _ = WireGuardConfig::delete();

    let output = output_result.map_err(|e| {
        VpnError::TunnelUpFailed(format!("Failed to spawn osascript: {e}"))
    })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(VpnError::TunnelUpFailed(format!(
            "wg-quick up failed: {stderr}"
        )))
    }
}

/// Tear down the active WireGuard VPN tunnel.
///
/// Executes `wg-quick down` via `osascript` with administrator privileges,
/// then explicitly zeroizes `key_pair` as a belt-and-suspenders measure
/// (the type already implements `ZeroizeOnDrop` for automatic zeroing on drop).
pub async fn tunnel_down(key_pair: &mut WireGuardKeyPair) -> Result<(), VpnError> {
    let script = build_tunnel_down_script();

    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(|e| VpnError::TunnelDownFailed(format!("Failed to spawn osascript: {e}")))?;

    // Explicitly zero the key pair regardless of tunnel teardown result.
    key_pair.zeroize();

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(VpnError::TunnelDownFailed(format!(
            "wg-quick down failed: {stderr}"
        )))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── Script builder tests (pure, no I/O) ─────────────────────────────────

    #[test]
    fn build_tunnel_up_script_contains_required_parts() {
        let dir = PathBuf::from("/usr/local/bin");
        let script = build_tunnel_up_script(&dir, CONFIG_PATH);

        assert!(
            script.contains("WG_QUICK_USERSPACE_IMPLEMENTATION=/usr/local/bin/wireguard-go"),
            "userspace impl env var missing: {script}"
        );
        assert!(
            script.contains("PATH=/usr/local/bin:$PATH"),
            "PATH override missing: {script}"
        );
        assert!(
            script.contains("/usr/local/bin/wg-quick up"),
            "wg-quick up missing: {script}"
        );
        assert!(
            script.contains(CONFIG_PATH),
            "config path missing: {script}"
        );
        assert!(
            script.contains("with administrator privileges"),
            "privilege escalation missing: {script}"
        );
    }

    #[test]
    fn build_tunnel_down_script_contains_required_parts() {
        let script = build_tunnel_down_script();

        assert!(
            script.contains("wg-quick down oh-my-vpn-wg0"),
            "wg-quick down command missing: {script}"
        );
        assert!(
            script.contains("with administrator privileges"),
            "privilege escalation missing: {script}"
        );
    }

    // ── Sidecar path resolution tests ──────────────────────────────────────

    #[test]
    fn resolve_sidecar_dir_returns_parent_of_current_exe() {
        let dir = resolve_sidecar_dir().expect("should resolve sidecar dir");
        // The resolved dir must be an existing directory (parent of test binary).
        assert!(dir.is_dir(), "sidecar dir should be a directory: {}", dir.display());
        // It should be the parent of the current executable.
        let exe = std::env::current_exe().unwrap();
        assert_eq!(dir, exe.parent().unwrap());
    }

    // ── Sidecar validation tests ─────────────────────────────────────────────

    #[test]
    fn validate_sidecar_binaries_fails_for_missing_directory() {
        let result = validate_sidecar_binaries(Path::new("/nonexistent/path-oh-my-vpn-test"));
        assert!(
            matches!(result, Err(VpnError::SidecarNotFound(_))),
            "expected SidecarNotFound, got {result:?}"
        );
    }

    #[test]
    fn validate_sidecar_binaries_succeeds_when_all_present() {
        let dir = std::env::temp_dir().join("oh-my-vpn-sidecar-test-all");
        fs::create_dir_all(&dir).expect("test dir should be creatable");

        for binary in &["wireguard-go", "wg", "wg-quick"] {
            fs::write(dir.join(binary), b"stub").expect("stub binary should be writable");
        }

        let result = validate_sidecar_binaries(&dir);

        // Clean up before asserting so a failure does not leave stale files.
        for binary in &["wireguard-go", "wg", "wg-quick"] {
            let _ = fs::remove_file(dir.join(binary));
        }

        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    #[test]
    fn validate_sidecar_binaries_fails_on_partial_presence() {
        let dir = std::env::temp_dir().join("oh-my-vpn-sidecar-test-partial");
        fs::create_dir_all(&dir).expect("test dir should be creatable");

        // Only create two of the three required binaries.
        fs::write(dir.join("wireguard-go"), b"stub").unwrap();
        fs::write(dir.join("wg"), b"stub").unwrap();
        // wg-quick intentionally absent.

        let result = validate_sidecar_binaries(&dir);

        // Clean up before asserting.
        let _ = fs::remove_file(dir.join("wireguard-go"));
        let _ = fs::remove_file(dir.join("wg"));

        assert!(
            matches!(result, Err(VpnError::SidecarNotFound(ref msg)) if msg.contains("wg-quick")),
            "expected SidecarNotFound mentioning wg-quick, got {result:?}"
        );
    }

    // ── Integration test (requires sudo + real system) ──────────────────────

    /// Full tunnel up/down cycle using loopback config.
    ///
    /// This test requires:
    /// - macOS with admin privileges (osascript authorization dialog)
    /// - Sidecar binaries in the same directory as the test binary
    /// - No real VPN server needed -- uses a loopback peer endpoint
    ///
    /// Run manually: `cargo test -- --ignored tunnel_up_down_cycle`
    #[tokio::test]
    #[ignore]
    async fn tunnel_up_down_cycle() {
        use std::process::Command as StdCommand;

        // Tunnel up with loopback config (no real server needed for interface creation).
        let up_result = tunnel_up(
            "127.0.0.1",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", // dummy 32-byte base64 key
            "10.99.99.1/32",
            "1.1.1.1",
        )
        .await;

        assert!(
            up_result.is_ok(),
            "tunnel_up failed: {:?}",
            up_result.err()
        );

        // Verify utun interface exists via ifconfig.
        let ifconfig = StdCommand::new("ifconfig")
            .output()
            .expect("ifconfig should be available");
        let ifconfig_output = String::from_utf8_lossy(&ifconfig.stdout);
        // wg-quick creates an interface named after the config file basename.
        assert!(
            ifconfig_output.contains("oh-my-vpn-wg0")
                || ifconfig_output.contains("utun"),
            "expected WireGuard interface in ifconfig output"
        );

        // Tunnel down.
        let mut key_pair = WireGuardKeyPair::generate();
        let down_result = tunnel_down(&mut key_pair).await;

        assert!(
            down_result.is_ok(),
            "tunnel_down failed: {:?}",
            down_result.err()
        );

        // Verify key pair is zeroed (all bytes should be 0 after zeroize).
        assert!(
            key_pair.private_key_base64().chars().all(|c| c == 'A' || c == '='),
            "key pair should be zeroed after tunnel_down"
        );
    }
}
