# Quality Attributes and Fitness Functions

Oh My VPN's quality attributes are derived from the PRD's non-functional requirements and the architectural constraints of a single-binary Tauri desktop app maintained by a solo developer. This document defines which attributes matter most, how to measure them, and when to measure them.

---

## 1. Attribute Selection

### A. Selection Criteria

Three factors drive attribute priority:

1. **Core value alignment** -- does the attribute protect the product's primary value proposition?
2. **Risk exposure** -- what is the cost of degradation in this attribute?
3. **Automation feasibility** -- can the attribute be measured automatically in CI?

### B. Selected Attributes

| Priority | Attribute | Selection Rationale | Phase |
| --- | --- | --- | --- |
| 1 | **Security** | Core value -- privacy by destruction, zero-log, ephemeral keys. A security failure invalidates the product's reason to exist | MVP |
| 2 | **Reliability** | Orphaned servers = billing leaks + security exposure. Single-user desktop app has no ops team to intervene -- automatic recovery is mandatory | MVP |
| 3 | **Performance** | Provisioning speed and app responsiveness are the primary UX differentiators vs manual CLI setup | MVP |
| 4 | **Maintainability** | Solo developer maintaining 3 cloud providers. Without modular boundaries, adding or updating a provider becomes a full-codebase risk | v1.1 |
| 5 | **Testability** | Security and reliability fitness functions depend on automated tests. Low testability undermines all other attributes | v1.1 |

---

## 2. Fitness Functions

### A. Security (MVP -- CI)

| # | Metric | Tool | Threshold | Frequency | NFR Ref |
| --- | --- | --- | --- | --- | --- |
| S-1 | Plaintext credential scan | `cargo deny` + custom grep in CI | 0 matches for API key patterns in source/config | Every CI run | NFR-SEC-1 |
| S-2 | WireGuard config file permission | Integration test (Rust `#[test]`) | File permission = 600; file deleted after tunnel setup | Every CI run | NFR-SEC-6 |
| S-3 | Tauri IPC command whitelist | Static analysis -- count `#[tauri::command]` vs allowlist in `tauri.conf.json` | Delta = 0 (every exposed command is explicitly whitelisted) | Every CI run | NFR-SEC-7 |
| S-4 | Known vulnerabilities in dependencies | `cargo audit` | 0 critical/high | Every CI run | -- |
| S-5 | DNS leak during active session | E2E test -- connect VPN, resolve domain, verify DNS server is VPN resolver | 0 leaks | Nightly / pre-release | NFR-SEC-3 |

### B. Reliability (MVP -- CI)

| # | Metric | Tool | Threshold | Frequency | NFR Ref |
| --- | --- | --- | --- | --- | --- |
| R-1 | Orphaned server detection rate | Integration test -- simulate crash, restart app, assert detection | 100% detection across all providers | Every CI run (mock) + Nightly (real API) | NFR-REL-1 |
| R-2 | Auto-cleanup on provisioning failure | Integration test -- inject failure mid-provision, assert server destroyed | 0 orphaned servers from failed provisioning | Every CI run (mock) | NFR-REL-2 |
| R-3 | Destruction retry with backoff | Unit test -- mock API failure, verify 3 retries with exponential backoff | ≥ 3 retries, backoff multiplier ≥ 2x | Every CI run | NFR-REL-3 |
| R-4 | Tunnel state reconciliation on crash | Integration test -- kill process during active session, restart, verify state | Tunnel state reconciled (no zombie tunnels) | Every CI run (mock) | NFR-REL-4 |

### C. Performance (MVP -- CI + Nightly)

| # | Metric | Tool | Threshold | Frequency | NFR Ref |
| --- | --- | --- | --- | --- | --- |
| P-1 | App launch to menu bar ready | Benchmark test (`cargo bench` or Tauri E2E) | ≤ 3 seconds (cold start, no onboarding) | Nightly | NFR-PERF-3 |
| P-2 | Region list load time | Integration test -- time from provider selection to region list rendered | ≤ 5 seconds | Nightly (real API) | NFR-PERF-4 |
| P-3 | Provisioning to VPN connected | E2E test -- full cycle with real cloud API | ≤ 120 seconds | Weekly (cost-gated) | NFR-PERF-1 |
| P-4 | Disconnect to destruction confirmed | E2E test -- time from disconnect click to API confirmation | ≤ 30 seconds | Weekly (cost-gated) | NFR-PERF-2 |

### D. Maintainability (v1.1 -- CI)

| # | Metric | Tool | Threshold | Frequency | NFR Ref |
| --- | --- | --- | --- | --- | --- |
| M-1 | Circular dependencies (Rust modules) | `cargo modules` or custom dep graph check | 0 circular dependencies | Every CI run | -- |
| M-2 | Function complexity | `clippy` with cognitive complexity lint | Complexity ≤ 15 per function | Every CI run | -- |
| M-3 | File length | Custom lint / CI script | ≤ 500 lines per `.rs` file | Every CI run | -- |
| M-4 | Provider trait compliance | Compile-time (Rust trait system) | All 3 providers implement `CloudProvider` trait | Every CI run (compile) | -- |

### E. Testability (v1.1 -- CI)

| # | Metric | Tool | Threshold | Frequency | NFR Ref |
| --- | --- | --- | --- | --- | --- |
| T-1 | Test coverage on critical paths | `cargo tarpaulin` or `cargo llvm-cov` | ≥ 80% on `server_lifecycle`, `vpn_manager`, `keychain_adapter` | Every CI run | -- |
| T-2 | Provider mock coverage | CI check -- each provider has mock implementation | 3/3 providers mocked | Every CI run | -- |

---

## 3. Fitness Function Type Classification

| # | Scope | Trigger | Result | Execution |
| --- | --- | --- | --- | --- |
| S-1 -- S-4 | Atomic | Triggered (CI) | Static (pass/fail) | Automated |
| S-5 | Holistic | Triggered (nightly) | Static | Automated |
| R-1 -- R-4 | Atomic | Triggered (CI) | Static | Automated |
| P-1 -- P-2 | Atomic | Triggered (nightly) | Dynamic (threshold may adjust) | Automated |
| P-3 -- P-4 | Holistic | Triggered (weekly) | Static | Automated (cost-gated) |
| M-1 -- M-4 | Atomic | Triggered (CI) | Static | Automated |
| T-1 -- T-2 | Atomic | Triggered (CI) | Static | Automated |

**Type definitions:**

- **Scope**: Atomic tests one attribute; Holistic tests attribute combinations (e.g., S-5 tests security + networking together)
- **Trigger**: Triggered runs on event (CI push, nightly schedule); cost-gated runs are limited by cloud API billing
- **Result**: Static has a fixed pass/fail boundary; Dynamic thresholds may shift as the system matures
- **Execution**: All fitness functions are automated -- manual review is not a fitness function

---

## 4. Frequency Strategy

```plain
Every CI Run          Nightly              Weekly
(every push)          (scheduled)          (cost-gated)
-----------------     -----------------    -----------------
S-1  Credential scan  S-5  DNS leak test   P-3  Full provisioning
S-2  Config perms     R-1  Real API orphan P-4  Full destruction
S-3  IPC whitelist    P-1  App launch
S-4  Cargo audit      P-2  Region load
R-1  Orphan (mock)
R-2  Auto-cleanup
R-3  Retry backoff
R-4  Tunnel state
M-1  Circular deps*
M-2  Complexity*
M-3  File length*
M-4  Trait compliance*
T-1  Coverage*
T-2  Mock coverage*

* = v1.1 phase
```

Weekly tests (P-3, P-4) involve real cloud provider API calls that incur costs. These run on a dedicated schedule with a cost cap -- skip execution if the monthly test budget is exhausted.

---

## 5. Review Cadence

- **Quarterly**: Reassess thresholds and attribute relevance
- **On new ADR**: Check if the decision affects any fitness function threshold
- **On provider addition**: Extend R-1, R-2, T-2 to cover the new provider
