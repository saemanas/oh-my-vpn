# Glossary

Domain-specific terms used throughout Oh My VPN documentation.

---

| Term | Definition |
| --- | --- |
| **Auto-cleanup** | Automatic destruction of partially created cloud resources (server, SSH key) when provisioning fails. Prevents orphaned servers and unexpected billing (FR-SL-4) |
| **boringtun** | Cloudflare's userspace WireGuard implementation in Rust. Considered but rejected for MVP in favor of wireguard-go ([ADR-0001](adr/0001-use-wireguard-go-with-wg-quick.md)) |
| **cloud-init** | Industry-standard tool for automating cloud instance initialization. Used to install and configure WireGuard on provisioned servers |
| **DNS leak** | When DNS queries bypass the VPN tunnel and reach the ISP's DNS resolver, exposing the user's real browsing activity |
| **Ed25519** | Elliptic curve digital signature algorithm. Used for ephemeral SSH key generation ([ADR-0004](adr/0004-ephemeral-ssh-keys-per-session.md)) and Tauri update binary signing ([ADR-0007](adr/0007-tauri-updater-with-github-releases.md)) |
| **Ephemeral key** | A cryptographic key pair generated per session and deleted after use. Applies to both WireGuard keys (NFR-SEC-2) and SSH keys ([ADR-0004](adr/0004-ephemeral-ssh-keys-per-session.md)) |
| **Ephemeral server** | A cloud instance created for a single VPN session and destroyed immediately after disconnection |
| **IPv6 leak** | When IPv6 traffic bypasses the VPN tunnel, potentially exposing the user's real IPv6 address |
| **Keychain** | macOS system service for securely storing passwords, API keys, certificates, and other sensitive credentials |
| **Network Extension** | macOS framework for creating VPN clients, content filters, and DNS proxies. Not required for MVP ([ADR-0003](adr/0003-no-network-extension-for-mvp.md)) |
| **Orphaned server** | A cloud instance that continues running without an active app session, typically caused by app crash or network loss during destruction |
| **osascript** | macOS command-line tool for executing AppleScript. Used to invoke sudo privilege escalation with a native authorization dialog for wg-quick ([ADR-0001](adr/0001-use-wireguard-go-with-wg-quick.md)) |
| **Provider** | A cloud infrastructure service (Hetzner, AWS, GCP) used to provision VPN servers |
| **ProviderTrait** | Rust trait that defines a common interface for all cloud providers. Each provider (Hetzner, AWS, GCP) implements this trait independently ([ADR-0002](adr/0002-use-rust-sdk-for-cloud-providers.md)) |
| **Provisioning** | The process of creating a cloud server, installing WireGuard via cloud-init, and configuring firewall rules |
| **Security Framework** | macOS API for accessing Keychain services, cryptographic operations, and secure credential storage |
| **Session** | The period from VPN connection establishment to disconnection and server destruction |
| **Tauri** | Framework for building lightweight desktop applications with a web frontend (TypeScript/HTML/CSS) and a Rust backend |
| **Tauri IPC** | Inter-Process Communication mechanism between Tauri's webview frontend and Rust backend. Commands are whitelisted for security (NFR-SEC-7) |
| **Tauri updater** | Built-in Tauri plugin for auto-update distribution with Ed25519 signature verification. Primary update channel with GitHub Releases as fallback ([ADR-0007](adr/0007-tauri-updater-with-github-releases.md)) |
| **TTL** | Time-to-live. Duration for which cached data (e.g., provider pricing) is considered fresh before refetching from the source API |
| **utun** | macOS virtual network tunnel interface. Created by wg-quick with root privileges to establish WireGuard tunnels |
| **wg-quick** | WireGuard CLI tool that automates tunnel setup -- creates utun devices, configures routes, sets DNS. Bundled inside the app ([ADR-0001](adr/0001-use-wireguard-go-with-wg-quick.md)) |
| **WireGuard** | Modern VPN protocol known for simplicity, high performance, and strong cryptography. Uses UDP for transport |
| **wireguard-go** | Official Go-based userspace WireGuard implementation. Bundled with wg-quick inside the app ([ADR-0001](adr/0001-use-wireguard-go-with-wg-quick.md)) |
