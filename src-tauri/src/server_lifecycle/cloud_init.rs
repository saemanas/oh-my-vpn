//! WireGuard cloud-init script builder.
//!
//! Builds a provider-agnostic bash script that provisions a fresh Ubuntu 24.04
//! server as a WireGuard VPN endpoint. The script is passed as cloud-init
//! user-data to the cloud provider's create_server API.

/// Build a cloud-init bash script that configures WireGuard on a fresh server.
///
/// The script:
/// 1. Installs `wireguard` and `wireguard-tools` packages
/// 2. Writes `/etc/wireguard/wg0.conf` with the server interface and client peer
/// 3. Enables IPv4 forwarding via sysctl
/// 4. Configures UFW firewall (allow SSH 22/tcp, WireGuard 51820/udp, deny incoming)
/// 5. Starts WireGuard via `systemctl enable --now wg-quick@wg0`
///
/// # Arguments
///
/// * `server_private_key` -- WireGuard server private key (base64-encoded)
/// * `client_public_key` -- WireGuard client public key (base64-encoded)
pub fn build_cloud_init(server_private_key: &str, client_public_key: &str) -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail

# -- Install WireGuard
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq wireguard wireguard-tools

# -- Write WireGuard server configuration
cat > /etc/wireguard/wg0.conf << 'WGEOF'
[Interface]
PrivateKey = {server_private_key}
Address = 10.0.0.1/24
ListenPort = 51820

[Peer]
PublicKey = {client_public_key}
AllowedIPs = 10.0.0.2/32
WGEOF

chmod 600 /etc/wireguard/wg0.conf

# -- Enable IP forwarding
sysctl -w net.ipv4.ip_forward=1
echo "net.ipv4.ip_forward=1" >> /etc/sysctl.d/99-wireguard.conf

# -- Configure firewall (UFW)
ufw allow 22/tcp
ufw allow 51820/udp
ufw --force enable

# -- Start WireGuard
systemctl enable --now wg-quick@wg0
"#,
        server_private_key = server_private_key,
        client_public_key = client_public_key,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SERVER_KEY: &str = "kFakeServerPrivateKeyBase64TestValue12345678=";
    const TEST_CLIENT_KEY: &str = "cFakeClientPublicKeyBase64TestValue123456789=";

    fn build_test_script() -> String {
        build_cloud_init(TEST_SERVER_KEY, TEST_CLIENT_KEY)
    }

    #[test]
    fn script_has_shebang() {
        let script = build_test_script();
        assert!(
            script.starts_with("#!/bin/bash"),
            "script should start with shebang: {script}"
        );
    }

    #[test]
    fn script_has_error_handling() {
        let script = build_test_script();
        assert!(
            script.contains("set -euo pipefail"),
            "script should have strict error handling: {script}"
        );
    }

    #[test]
    fn script_installs_wireguard() {
        let script = build_test_script();
        assert!(
            script.contains("apt-get install") && script.contains("wireguard"),
            "script should install wireguard package: {script}"
        );
    }

    #[test]
    fn script_contains_server_private_key() {
        let script = build_test_script();
        assert!(
            script.contains(TEST_SERVER_KEY),
            "script should contain the server private key: {script}"
        );
    }

    #[test]
    fn script_contains_client_public_key() {
        let script = build_test_script();
        assert!(
            script.contains(TEST_CLIENT_KEY),
            "script should contain the client public key: {script}"
        );
    }

    #[test]
    fn script_has_interface_section() {
        let script = build_test_script();
        assert!(
            script.contains("[Interface]"),
            "script should have [Interface] section: {script}"
        );
        assert!(
            script.contains("Address = 10.0.0.1/24"),
            "script should set server tunnel address: {script}"
        );
        assert!(
            script.contains("ListenPort = 51820"),
            "script should set WireGuard listen port: {script}"
        );
    }

    #[test]
    fn script_has_peer_section() {
        let script = build_test_script();
        assert!(
            script.contains("[Peer]"),
            "script should have [Peer] section: {script}"
        );
        assert!(
            script.contains("AllowedIPs = 10.0.0.2/32"),
            "script should set client allowed IPs: {script}"
        );
    }

    #[test]
    fn script_enables_ip_forwarding() {
        let script = build_test_script();
        assert!(
            script.contains("net.ipv4.ip_forward=1"),
            "script should enable IP forwarding: {script}"
        );
    }

    #[test]
    fn script_configures_firewall() {
        let script = build_test_script();
        assert!(
            script.contains("ufw allow 22/tcp"),
            "script should allow SSH: {script}"
        );
        assert!(
            script.contains("ufw allow 51820/udp"),
            "script should allow WireGuard: {script}"
        );
        assert!(
            script.contains("ufw --force enable"),
            "script should enable firewall: {script}"
        );
    }

    #[test]
    fn script_starts_wireguard() {
        let script = build_test_script();
        assert!(
            script.contains("systemctl enable --now wg-quick@wg0"),
            "script should start WireGuard service: {script}"
        );
    }

    #[test]
    fn script_sets_config_permissions() {
        let script = build_test_script();
        assert!(
            script.contains("chmod 600 /etc/wireguard/wg0.conf"),
            "script should restrict config file permissions: {script}"
        );
    }

    #[test]
    fn interface_comes_before_peer() {
        let script = build_test_script();
        let interface_pos = script.find("[Interface]").expect("[Interface] should exist");
        let peer_pos = script.find("[Peer]").expect("[Peer] should exist");
        assert!(
            interface_pos < peer_pos,
            "[Interface] should come before [Peer]"
        );
    }
}
