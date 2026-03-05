---
task: "Server Lifecycle IPC Commands"
milestone: "M4"
module: "M4.5"
created_at: "2026-03-05T14:15:00+07:00"
status: "completed"
branch: "feat/server-lifecycle-ipc"
---

> **Status**: Completed at 2026-03-05T14:22:00+07:00
> **Branch**: feat/server-lifecycle-ipc

# PLAN -- M4.5: Server Lifecycle IPC Commands

## 1. Context

### A. Problem Statement

M4.1--M4.4 completed all backend domain logic (connect, disconnect, orphan detection/resolution, session tracking). The IPC layer has stub handlers returning `NOT_IMPLEMENTED` for 4 out of 5 commands. These stubs must be wired to the domain methods so the frontend (M5) can invoke them.

### B. Current State

- `src-tauri/src/ipc/server.rs`: `connect` implemented, `disconnect` / `check_orphaned_servers` / `resolve_orphaned_server` are stubs
- `src-tauri/src/ipc/session.rs`: `get_session_status` is a stub
- `src-tauri/capabilities/default.json`: only `core:default` and `opener:default` -- no IPC command permissions
- All domain methods exist and have unit tests:
  - `ServerLifecycle::disconnect(&self, &Mutex<ProviderRegistry>) -> Result<(), LifecycleError>`
  - `ServerLifecycle::check_orphaned_servers(&self, &Mutex<ProviderRegistry>) -> Result<Vec<OrphanedServer>, LifecycleError>`
  - `ServerLifecycle::resolve_orphaned_server(&self, &str, OrphanAction, &Mutex<ProviderRegistry>) -> Result<Option<SessionStatus>, LifecycleError>`
  - `SessionTracker::get_status(&self) -> Result<Option<SessionStatus>, SessionError>`
- Error conversions `From<LifecycleError> for AppError` and `From<SessionError> for AppError` already implemented in `error.rs`
- Types `OrphanAction`, `OrphanedServer`, `SessionStatus` defined in `types.rs` and `session_tracker.rs`

### C. Constraints

- Follow the established IPC pattern from `connect` (in `server.rs`) and `provider.rs`: extract `tauri::State`, validate at boundary, delegate to domain, convert error via `From`
- Rust signatures must match API Design §4.C and §4.D
- Tauri v2 capabilities must whitelist all IPC commands

### D. Input Sources

- Milestone doc: `docs/milestone/2026-03-04-1726-milestone.md` -- M4.5 acceptance criteria
- API Design: `docs/api-design/2026-03-04-1726-api-design.md` -- §3.B, §3.C, §4.C, §4.D

### E. Verified Facts

1. `connect` IPC pattern verified: `lifecycle: tauri::State<'_, ServerLifecycle>`, `registry: tauri::State<'_, Mutex<ProviderRegistry>>` -- same pattern applies to all server IPC commands
2. `disconnect` API design signature: `async fn disconnect() -> Result<(), AppError>` -- but needs `tauri::State` params for `ServerLifecycle` and `ProviderRegistry` (not shown in API design because Tauri state params are invisible to the frontend)
3. `resolve_orphaned_server` takes `OrphanAction` enum (not raw string) -- `OrphanAction` has `#[serde(rename_all = "lowercase")]` so frontend sends `"destroy"` or `"reconnect"`
4. `get_session_status` accesses `lifecycle.session_tracker.get_status()` -- `SessionTracker` is a public field on `ServerLifecycle`
5. `From<LifecycleError> for AppError` handles all lifecycle error variants including `NoActiveSession`, `DestructionFailed`, `OrphanDetectionFailed`, `OrphanReconnectFailed`
6. `invoke_handler` in `lib.rs` already registers all 5 server/session commands -- no changes needed there

### F. Unverified Assumptions

None -- all interfaces verified in codebase.

---

## 2. Architecture

No structural decisions needed. This is a pure wiring task using the established IPC delegation pattern:

```plain
IPC Command (tauri::command)
  → Extract tauri::State<ServerLifecycle> + tauri::State<Mutex<ProviderRegistry>>
  → Input validation at boundary
  → Delegate to domain method
  → Error auto-converted via From<LifecycleError/SessionError> for AppError
```

The pattern is identical to the existing `connect` command and all `provider.rs` commands. No new modules, types, or error variants needed.

---

## 3. Steps

### Step 1: Implement disconnect, check_orphaned_servers, resolve_orphaned_server in server.rs

- [x] **Status**: completed at 2026-03-05T14:20:00+07:00
- **Scope**: `src-tauri/src/ipc/server.rs`
- **Dependencies**: none
- **Description**: Replace the 3 NOT_IMPLEMENTED stubs with real implementations that delegate to `ServerLifecycle` domain methods. Follow the same pattern as the existing `connect` command.
- **Acceptance Criteria**:
  - `disconnect` extracts `ServerLifecycle` and `Mutex<ProviderRegistry>` from state, delegates to `lifecycle.disconnect(&registry)`
  - `check_orphaned_servers` extracts same state, delegates to `lifecycle.check_orphaned_servers(&registry)`
  - `resolve_orphaned_server` extracts same state, takes `server_id: String` and `action: OrphanAction`, delegates to `lifecycle.resolve_orphaned_server(&server_id, action, &registry)`
  - All errors auto-convert via existing `From<LifecycleError> for AppError`
  - No raw string error codes -- use `From` conversion only

### Step 2: Implement get_session_status in session.rs

- [x] **Status**: completed at 2026-03-05T14:20:00+07:00
- **Scope**: `src-tauri/src/ipc/session.rs`
- **Dependencies**: none
- **Description**: Replace the NOT_IMPLEMENTED stub with a real implementation that delegates to `SessionTracker::get_status()`.
- **Acceptance Criteria**:
  - `get_session_status` extracts `ServerLifecycle` from state, delegates to `lifecycle.session_tracker.get_status()`
  - Returns `Option<SessionStatus>` (None when no active session)
  - Error auto-converts via existing `From<SessionError> for AppError`

### Step 3: Update build.rs AppManifest and Tauri capabilities

- [x] **Status**: completed at 2026-03-05T14:21:00+07:00
- **Scope**: `src-tauri/build.rs`, `src-tauri/capabilities/default.json`
- **Dependencies**: Step 1, Step 2
- **Description**: Configure `build.rs` with `AppManifest` to auto-generate `allow-<command>` / `deny-<command>` permission identifiers for all 11 IPC commands. Then reference those permissions in `default.json` so the frontend can invoke them. Without `AppManifest`, custom app commands are unrestricted (no IPC whitelist enforcement).
- **Acceptance Criteria**:
  - `build.rs` uses `tauri_build::try_build` with `AppManifest::new().commands(&[...])` listing all 11 commands
  - `default.json` references the auto-generated `allow-<command>` permissions for all 11 commands
  - Only whitelisted commands are accessible from the frontend (NFR-SEC-7 scaffold)

### Step 4: Compile and test verification

- [x] **Status**: completed at 2026-03-05T14:22:00+07:00
- **Scope**: full project
- **Dependencies**: Step 1, Step 2, Step 3
- **Description**: Run `cargo check` to verify compilation and `cargo test` to verify existing tests still pass.
- **Acceptance Criteria**:
  - `cargo check` passes with no errors
  - `cargo test` passes (all existing unit tests)
  - No new warnings introduced

---

## 4. Execution Strategy

| Step | Chain | Rationale |
| --- | --- | --- |
| 1 | Direct | 3 stub replacements in one file, established pattern |
| 2 | Direct | 1 stub replacement, trivial |
| 3 | Direct | build.rs + capabilities JSON update |
| 4 | Direct | Compile + test commands |

**Execution order**: Step 1 and Step 2 are independent (parallel-eligible but executed sequentially per policy). Step 3 after both. Step 4 last.

```plain
Step 1 → Step 2 → Step 3 → Step 4
```

**Estimated complexity**: All Trivial (< 5K tokens each)

**Risk flags**: None

---
