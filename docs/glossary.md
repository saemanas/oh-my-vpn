# Glossary

Domain-specific terms used throughout Oh My VPN documentation.

---

| Term | Definition |
| --- | --- |
| **boringtun** | Cloudflare's userspace WireGuard implementation in Rust. Alternative to the kernel-level WireGuard client |
| **cloud-init** | Industry-standard tool for automating cloud instance initialization. Used to install and configure WireGuard on provisioned servers |
| **DNS leak** | When DNS queries bypass the VPN tunnel and reach the ISP's DNS resolver, exposing the user's real browsing activity |
| **Ephemeral server** | A cloud instance created for a single VPN session and destroyed immediately after disconnection |
| **IPv6 leak** | When IPv6 traffic bypasses the VPN tunnel, potentially exposing the user's real IPv6 address |
| **Keychain** | macOS system service for securely storing passwords, API keys, certificates, and other sensitive credentials |
| **Network Extension** | macOS framework for creating VPN clients, content filters, and DNS proxies. May require Apple Developer Program entitlement |
| **Orphaned server** | A cloud instance that continues running without an active app session, typically caused by app crash or network loss during destruction |
| **Provider** | A cloud infrastructure service (Hetzner, AWS, GCP) used to provision VPN servers |
| **Provisioning** | The process of creating a cloud server, installing WireGuard via cloud-init, and configuring firewall rules |
| **Security Framework** | macOS API for accessing Keychain services, cryptographic operations, and secure credential storage |
| **Session** | The period from VPN connection establishment to disconnection and server destruction |
| **Tauri** | Framework for building lightweight desktop applications with a web frontend (TypeScript/HTML/CSS) and a Rust backend |
| **Tauri IPC** | Inter-Process Communication mechanism between Tauri's webview frontend and Rust backend. Commands are whitelisted for security |
| **WireGuard** | Modern VPN protocol known for simplicity, high performance, and strong cryptography. Uses UDP for transport |
