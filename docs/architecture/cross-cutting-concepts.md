# Cross-Cutting Concepts

Patterns and strategies that span multiple containers in Oh My VPN. These are not localized to a single module -- they affect the system as a whole.

---

## 1. Credential Security

All sensitive credentials (cloud provider API keys) flow through a single path: the Keychain Adapter. No other container reads or writes credentials directly.

```mermaid
flowchart LR
    user([User]) -->|enters API key| ui[Menu Bar UI]
    ui -->|IPC command| tauri[Tauri Core]
    tauri -->|store key| adapter[Keychain Adapter]
    adapter -->|write| keychain[(macOS Keychain)]

    provider[Provider Manager] -->|read key| adapter
    adapter -->|retrieve| keychain
```

### A. Rules

- API keys are **never** stored in files, environment variables, or application memory beyond the immediate operation (NFR-SEC-1)
- WireGuard keys are **ephemeral** -- generated per session, held in memory during the session, deleted on teardown (NFR-SEC-2)
- WireGuard config files have permission `600` and are deleted immediately after tunnel establishment (NFR-SEC-6)

---

## 2. Error Handling and Retry

Oh My VPN follows a **fail-fast with graceful recovery** pattern. Errors are detected early, surfaced clearly, and recovered automatically where possible.

```mermaid
flowchart TD
    op[Operation Attempt] --> result{Success?}
    result -->|Yes| done[Complete]
    result -->|No| classify{Error Type?}

    subgraph transient["Transient Error"]
        direction TB
        t1[Retry up to 3x<br/>with exponential backoff]
        t2{All retries<br/>exhausted?}
        t1 --> t2
        t2 -->|No| t3[Retry operation]
        t2 -->|Yes| t4[Auto-cleanup<br/>+ notify user]
    end

    subgraph permanent["Permanent Error"]
        direction TB
        p1[No retry]
        p1 --> p2[Auto-cleanup<br/>+ notify user<br/>with specific error]
    end

    subgraph rateLimited["Rate Limited"]
        direction TB
        r1[Backoff with<br/>provider wait time]
        r1 --> r2[Notify user of delay]
        r2 --> r3[Retry operation]
    end

    classify -->|Transient| t1
    classify -->|Permanent| p1
    classify -->|Rate Limited| r1
```

### A. Transient Errors

Network timeouts, cloud API 5xx responses, WireGuard handshake failures.

- Retry up to 3 times with exponential backoff (NFR-REL-3)
- Auto-cleanup partial resources on final failure (FR-SL-4)

### B. Permanent Errors

Invalid API key, insufficient permissions, unsupported region.

- No retry -- fail fast with specific error message (NFR-INT-2)
- Guide user to resolution (e.g., "Check API key permissions")

### C. Rate Limiting

Cloud API 429 responses.

- Backoff with provider-specific wait time (NFR-INT-3)
- Notify user that the operation is delayed, not failed

---

## 3. Orphaned Server Recovery

An orphaned server is a cloud instance that exists without an active app session -- caused by app crash, force-quit, or network loss during destruction. This is a critical cost and security risk.

```mermaid
flowchart TD
    launch[App Launch] --> check{Persisted<br/>Server State?}
    check -->|No| ready[Menu Bar Ready]
    check -->|Yes| query[Query Cloud API<br/>for Each Provider]

    query --> exists{Server<br/>Still Exists?}
    exists -->|No| clearState[Clear Stale State] --> ready
    exists -->|Yes| prompt[Notify User:<br/>Orphaned Server Found]

    prompt --> choice{User Choice}
    choice -->|Destroy| destroy[Destroy Server] --> clearState
    choice -->|Reconnect| reconnect[Re-establish<br/>WireGuard Tunnel]
```

### A. Detection Strategy

On every app launch, the Session Tracker checks for persisted server state (server ID, provider, region). If state exists, the Provider Manager queries the cloud API to verify the server still exists. This ensures 100% detection rate (NFR-REL-1).

### B. State Persistence

Minimal state is persisted to detect orphans:

| Field | Purpose |
| --- | --- |
| `serverId` | Cloud instance identifier |
| `provider` | Which cloud provider (Hetzner/AWS/GCP) |
| `region` | Server region |
| `createdAt` | Provisioning timestamp |
| `hourlyCost` | For cost estimation |

This state is cleared on successful disconnection and server destruction.

---

## 4. DNS and IPv6 Leak Prevention

During an active VPN session, all network traffic must route through the WireGuard tunnel. Leaks expose the user's real IP address, defeating the core privacy value.

### A. DNS Leak Prevention

- All DNS queries route through the VPN tunnel (FR-VC-5, NFR-SEC-3)
- The WireGuard config sets `DNS` to the VPN server's resolver
- System DNS settings are restored on disconnection

### B. IPv6 Leak Prevention

- IPv6 traffic is disabled or tunneled during active session (FR-VC-6, NFR-SEC-4)
- Implementation: disable IPv6 at the network interface level or route all IPv6 through the tunnel

---

## 5. macOS Notifications

Status changes are communicated via macOS native notifications (FR-MN-2). This is a cross-cutting concern because multiple containers trigger notifications:

| Event | Source Container | Notification |
| --- | --- | --- |
| Server provisioned | Server Lifecycle | "VPN server ready" |
| VPN connected | VPN Manager | "VPN connected -- {region}" |
| VPN disconnected | VPN Manager | "Disconnected, server destroyed" |
| Orphaned server found | Session Tracker | "Active server detected from previous session" |
| Provisioning failed | Server Lifecycle | "Server creation failed -- {reason}" |
| API rate limited | Provider Manager | "Cloud API rate limited -- retrying" |

---

## 6. Cloud-Init Strategy

Server provisioning uses cloud-init to automate WireGuard installation and configuration. This is a cross-cutting concern because it involves the Server Lifecycle, VPN Manager, and Provider Manager.

### A. cloud-init Script Responsibilities

1. Install WireGuard package
2. Configure WireGuard interface with server private key and client public key
3. Configure firewall rules (allow WireGuard UDP port only)
4. Enable IP forwarding
5. Start WireGuard service

### B. Provider Variation

Each cloud provider may require slightly different cloud-init scripts (Risk R-1):

| Provider | Variation |
| --- | --- |
| Hetzner | Standard cloud-init, Ubuntu/Debian base image |
| AWS | User-data script, Amazon Linux or Ubuntu AMI, Security Groups for firewall |
| GCP | Startup script metadata, firewall rules via Compute Engine API |

Provider-specific scripts are maintained independently and tested per provider (Risk R-1 mitigation).
