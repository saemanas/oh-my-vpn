# Deployment View

Oh My VPN operates across two environments: the user's macOS machine (where the Tauri app runs) and ephemeral cloud servers (where WireGuard runs). This document maps containers to infrastructure nodes.

---

## 1. Deployment Diagram

```mermaid
flowchart TD
    subgraph macOs["User's macOS Machine <i>[macOS 13+]</i>"]
        subgraph tauriApp["Oh My VPN.app <i>[Tauri Bundle]</i>"]
            menuBarUi["<b>Menu Bar UI</b><br/><i>[TypeScript, Webview]</i><br/>User interface"]
            tauriCore["<b>Tauri Core</b><br/><i>[Rust]</i><br/>IPC bridge"]
            providerManager["<b>Provider Manager</b><br/><i>[Rust]</i><br/>Cloud API abstraction"]
            serverLifecycle["<b>Server Lifecycle</b><br/><i>[Rust]</i><br/>Provisioning orchestration"]
            vpnManager["<b>VPN Manager</b><br/><i>[Rust]</i><br/>WireGuard tunnel management"]
            sessionTracker["<b>Session Tracker</b><br/><i>[Rust]</i><br/>Session state tracking"]
            keychainAdapter["<b>Keychain Adapter</b><br/><i>[Rust]</i><br/>Credential access"]
        end
        subgraph osServices["macOS Services"]
            keychain["<b>Keychain</b><br/><i>[Security Framework]</i><br/>Encrypted credential storage"]
        end
    end

    subgraph cloud["Cloud Provider <i>[Hetzner / AWS / GCP]</i>"]
        wireguardServer["<b>WireGuard</b><br/><i>[cloud-init configured]</i><br/>VPN endpoint with firewall rules"]
    end

    vpnManager -->|"WireGuard tunnel (UDP)"| wireguardServer
    providerManager -->|"Provisions/destroys (HTTPS)"| cloud
    keychainAdapter -->|"Reads/writes keys"| keychain

    classDef appNode fill:#438dd5,stroke:#3c7fc0,color:#fff
    classDef osNode fill:#999,stroke:#888,color:#fff
    classDef cloudNode fill:#999,stroke:#888,color:#fff

    class menuBarUi,tauriCore,providerManager,serverLifecycle,vpnManager,sessionTracker,keychainAdapter appNode
    class keychain osNode
    class wireguardServer cloudNode
```

---

## 2. Infrastructure Nodes

### A. User's macOS Machine

| Attribute | Value |
| --- | --- |
| OS | macOS 13+ (Ventura or later) |
| Runtime | Tauri app bundle (.app) |
| Distribution | Direct download (v1.0), `brew install` (v2.0) |
| Update mechanism | Tauri built-in updater (OQ-6) or manual download |
| Privileges | Admin (sudo) required for wg-quick tunnel creation ([ADR-0001](../adr/0001-use-wireguard-go-with-wg-quick.md), [ADR-0003](../adr/0003-no-network-extension-for-mvp.md)) |

The entire Tauri application runs as a single process on the user's machine. All Rust containers are in-process modules -- not separate services.

### B. Ephemeral VPN Server

| Attribute | Value |
| --- | --- |
| Providers | Hetzner Cloud, AWS EC2, GCP Compute Engine |
| Lifecycle | Created on "Connect", destroyed on "Disconnect" |
| Configuration | cloud-init script installs WireGuard, configures firewall |
| Instance type | Cheapest available (e.g., Hetzner CX22, AWS t3.nano, GCP e2-micro) |
| Firewall rules | WireGuard UDP port only (NFR-SEC-5) |
| Persistence | None -- ephemeral by design. No data survives destruction |

### C. Network Topology

```mermaid
flowchart LR
    subgraph macOs["User's macOS"]
        app["Oh My VPN.app<br/>(WireGuard client)"]
        keychain[(macOS Keychain)]
        app --> keychain
    end

    cloudApi["Cloud API<br/>(Hetzner, AWS, GCP)"]
    wireguardServer["WireGuard Server<br/>(ephemeral)"]

    app -->|HTTPS| cloudApi
    cloudApi -->|provisions| wireguardServer
    app -->|UDP tunnel| wireguardServer
```

---

## 3. Deployment Characteristics

### A. No Server-Side Infrastructure

Oh My VPN has **no backend server** of its own. The app communicates directly with cloud provider APIs. This means:

- Zero ongoing infrastructure cost for the developer
- No single point of failure beyond the cloud providers themselves
- Users own their entire data path

### B. Ephemeral by Design

Cloud servers exist only during active VPN sessions. On disconnect, the server is destroyed and all data is deleted. This is a core security property (NFR-SEC-2, US-PRI-2).

### C. Multi-Provider Resilience

Supporting three cloud providers (Hetzner, AWS, GCP) means:

- If one provider has an outage, users can switch to another
- Regional coverage is maximized across providers
- Cost competition benefits the user

### D. Environment Summary

| Environment | Purpose | Lifetime |
| --- | --- | --- |
| User's macOS | App runtime, credential storage, VPN client | Permanent (app installed) |
| Cloud VPN server | WireGuard endpoint | Ephemeral (minutes to hours) |
| Cloud provider API | Server management | Always available (external) |
