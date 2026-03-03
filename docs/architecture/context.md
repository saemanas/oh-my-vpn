# Context and Scope

Oh My VPN is a macOS menu bar application that automates on-demand VPN server provisioning. Users create, connect to, and destroy their own WireGuard VPN servers across multiple cloud providers in one click.

This document defines the system boundary -- what Oh My VPN is, who interacts with it, and what external systems it depends on.

---

## 1. System Context

```mermaid
flowchart TD
    user["👤 <b>User</b><br/>Creates, connects to, and<br/>destroys VPN servers"]
    ohMyVpn["<b>Oh My VPN</b><br/><i>[macOS menu bar app]</i><br/>Provisions on-demand servers<br/>and establishes WireGuard tunnels"]
    cloudApi["<b>Cloud Provider API</b><br/><i>[External]</i><br/>Hetzner, AWS, GCP"]
    keychain["<b>macOS Keychain</b><br/><i>[External]</i><br/>Credential storage"]

    user -->|"GUI"| ohMyVpn
    ohMyVpn -->|"HTTPS"| cloudApi
    ohMyVpn -->|"Security Framework"| keychain

    classDef system fill:#438dd5,stroke:#3c7fc0,color:#fff
    classDef person fill:#08427b,stroke:#073b6f,color:#fff
    classDef external fill:#999,stroke:#888,color:#fff

    class user person
    class ohMyVpn system
    class cloudApi,keychain external
```

WireGuard is not an external system -- it is a protocol and library (boringtun) bundled inside the application. The individual cloud providers (Hetzner, AWS, GCP) are abstracted as a single external system at the context level; their differences are visible at the container level in [containers.md](containers.md).

---

## 2. External Actors

| Actor | Type | Interaction | Protocol |
| --- | --- | --- | --- |
| User | Person | Manages VPN sessions via menu bar UI | GUI (Tauri webview) |
| Cloud Provider API | External System | Server CRUD, region/pricing queries (Hetzner, AWS, GCP) | HTTPS REST |
| macOS Keychain | External System | Credential storage and retrieval | macOS Security Framework |

---

## 3. Key Boundaries

### A. Inside the System

- Tauri application (TypeScript frontend + Rust backend)
- Provider abstraction layer (unified interface for Hetzner, AWS, GCP)
- WireGuard integration (key generation, tunnel management via boringtun)
- Session state tracking (connected IP, elapsed time, cost)
- Orphaned server detection and recovery

### B. Outside the System

- Cloud provider account management (sign-up, billing, IAM)
- macOS Keychain encryption (delegated to OS)
- Network Extension entitlement (open question OQ-3 from PRD)

---

## 4. Open Decisions

These items from the PRD affect the system boundary and require ADRs:

| PRD Ref | Question | Impact |
| --- | --- | --- |
| OQ-1 | WireGuard via boringtun (userspace) or system client? | Determines WireGuard dependency type |
| OQ-2 | Direct HTTP API calls or CLI tool wrapping (hcloud, aws, gcloud)? | Determines cloud provider integration pattern |
| OQ-3 | Is macOS Network Extension entitlement required? | May add Apple Developer Program as external dependency |
| OQ-7 | Ephemeral SSH key strategy for provisioning? | Affects key management boundary |
