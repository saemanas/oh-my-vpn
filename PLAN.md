---
task: "Implement Preferences Store module"
milestone: "M1"
module: "M1.3"
created_at: "2026-03-04T20:23:45+07:00"
status: "completed"
branch: "feature/preferences-store"
---

> **Status**: Completed at 2026-03-04T20:48:00+07:00
> **Branch**: feature/preferences-store

# PLAN -- M1.3: Preferences Store

## 1. Context

### A. Problem Statement

The Preferences Store module persists user settings (last-used provider/region, notifications, keyboard shortcut) as a JSON file in the Tauri app data directory. It must support atomic writes (crash-safe), corrupt file recovery (backup + defaults), and schema versioning for future migrations.

### B. Current State

- `src-tauri/src/preferences_store.rs` exists with only a doc comment -- no implementation
- `keychain_adapter.rs` (M1.2) is complete and establishes the project pattern: stateless facade, dedicated error enum, `#[cfg(test)]` module with `#[ignore]` for integration tests
- `types.rs` has the `Provider` enum with serde support (`rename_all = "lowercase"`)
- `error.rs` (M1.4) is not yet implemented -- PreferencesStore defines its own error type
- `serde` (1) and `serde_json` (1) are already in `Cargo.toml`
- `lib.rs` already declares `mod preferences_store` with `#[allow(unused)]`

### C. Constraints

- Tauri v2 path API: `app_handle.path().app_data_dir()` returns `~/Library/Application Support/com.saemanas.oh-my-vpn/`
- File location: `{app_data_dir}/preferences.json`
- Atomic writes: write to `.preferences.tmp.json`, then `fs::rename` (POSIX atomic on same filesystem)
- Corrupt file backup: rename to `preferences.backup.json` before recreating defaults
- No new crate dependencies needed -- `serde`, `serde_json`, `std::fs` suffice

### D. Verified Facts

| # | Fact | Evidence |
| --- | --- | --- |
| 1 | `preferences_store.rs` exists with doc comment only | File read |
| 2 | KeychainAdapter uses stateless facade pattern with dedicated error enum | `keychain_adapter.rs` |
| 3 | `serde`, `serde_json` already in Cargo.toml | `Cargo.toml` |
| 4 | `Provider` enum in `types.rs` with serde lowercase | `types.rs` |
| 5 | `error.rs` has doc comment only (M1.4 pending) | File read |
| 6 | Tauri v2: `app_handle.path().app_data_dir()` → `~/Library/Application Support/com.saemanas.oh-my-vpn/` | Tauri v2 docs |
| 7 | `std::fs::rename` is atomic on macOS same filesystem | POSIX spec |
| 8 | `lib.rs` already declares `mod preferences_store` | `lib.rs` |

### E. Unverified Assumptions

| # | Assumption | Risk | Fallback |
| --- | --- | --- | --- |
| 1 | M1.4 `AppError` will add `From<PreferencesError>` later | Low | Self-contained `PreferencesError` works standalone; conversion added in M1.4 |

---

## 2. Architecture

### A. Module Structure

```
PreferencesStore (struct)
├── data_dir: PathBuf                     -- Tauri app data directory
├── new(data_dir: PathBuf) -> Self        -- constructor
├── load(&self) -> Result<UserPreferences, PreferencesError>
│   ├── file missing → create defaults, atomic write, return defaults
│   ├── file corrupt → backup to preferences.backup.json, recreate defaults
│   └── schema outdated → sequential migration (v1 → v2 → ...)
├── save(&self, prefs: &UserPreferences) -> Result<(), PreferencesError>
│   └── atomic write: write .preferences.tmp.json → rename to preferences.json
└── file_path(&self) -> PathBuf           -- {data_dir}/preferences.json

UserPreferences (struct, Serialize, Deserialize, Debug, Clone, PartialEq)
  #[serde(rename_all = "camelCase")]      -- JSON keys match data model (camelCase)
├── schema_version: u32                   -- starts at 1
├── last_provider: Option<Provider>       -- from types.rs (data model says String, but Provider enum is type-safe and serializes to same JSON)
├── last_region: Option<String>
├── notifications_enabled: bool           -- default: true
└── keyboard_shortcut: Option<String>

PreferencesError (enum, Debug)
├── ReadFailed(String)
├── WriteFailed(String)
├── ParseFailed(String)
└── MigrationFailed(String)
```

### B. Decisions

| Decision | Choice | Rationale (Principle) |
| --- | --- | --- |
| Struct with `data_dir` field | `PreferencesStore { data_dir }` | Explicit over Implicit -- path is injected, not hidden |
| No in-memory cache | Read/write disk every call | Single Responsibility -- no cache invalidation concerns; file is <1KB |
| Own error enum | `PreferencesError` | Dependency Inversion -- no dependency on M1.4's `AppError` |
| Atomic write via rename | tmp file + `fs::rename` | Fail Fast -- no partial writes on crash |
| Corrupt recovery | backup + defaults | Fail Fast -- clear error, predictable recovery |

### C. Boundaries

- PreferencesStore owns `preferences.json` exclusively -- no other module reads/writes this file
- Server Lifecycle (M4) and IPC (M6.2) will call PreferencesStore methods -- they depend on this module, not the reverse
- `UserPreferences` struct is public for consumers; `PreferencesError` is public for error handling

---

## 3. Steps

### Step 1: UserPreferences struct + PreferencesError enum

- [x] **Status**: completed at 2026-03-04T20:37:00+07:00
- **Scope**: `src-tauri/src/preferences_store.rs` -- type definitions only
- **Dependencies**: none
- **Description**: Define `UserPreferences` struct with serde derives, `#[serde(rename_all = "camelCase")]` (JSON keys match data model), and `Default` impl (schema_version=1, notifications_enabled=true, all optional fields=None). Define `PreferencesError` enum with Display and Error impls. Define the `CURRENT_SCHEMA_VERSION` constant.
- **Acceptance Criteria**:
  - `UserPreferences` has all 5 fields matching data model §4.B
  - `#[serde(rename_all = "camelCase")]` applied -- JSON output uses camelCase keys
  - `Default::default()` returns schema_version=1, notifications_enabled=true, rest=None
  - `PreferencesError` has 4 variants with `Display` and `std::error::Error`
  - `CURRENT_SCHEMA_VERSION` constant set to `1`
  - `cargo check` passes

### Step 2: PreferencesStore core (new, file_path, load, save)

- [x] **Status**: completed at 2026-03-04T20:42:00+07:00
- **Scope**: `src-tauri/src/preferences_store.rs` -- implementation
- **Dependencies**: Step 1
- **Description**: Implement `PreferencesStore` struct with `data_dir` field. `new(data_dir)` constructor. `file_path()` returns `{data_dir}/preferences.json`. `save()` writes to `.preferences.tmp.json` then renames atomically, creating `data_dir` if needed. `load()` handles 4 cases: (1) missing file → create defaults, (2) valid file → return parsed, (3) corrupt file → backup to `preferences.backup.json` + recreate defaults, (4) outdated schema → run sequential migration + save upgraded.
- **Acceptance Criteria**:
  - `save()` writes pretty JSON to tmp file then renames atomically
  - `save()` creates `data_dir` if it does not exist (`create_dir_all`)
  - `load()` missing file → writes defaults → returns defaults
  - `load()` corrupt file → renames to `preferences.backup.json` → writes defaults → returns defaults
  - `load()` valid file → returns parsed UserPreferences
  - `load()` outdated schema_version → migrates sequentially → saves → returns
  - `cargo check` passes

### Step 3: Unit tests

- [x] **Status**: completed at 2026-03-04T20:48:00+07:00
- **Scope**: `src-tauri/src/preferences_store.rs` -- `#[cfg(test)]` module
- **Dependencies**: Step 2
- **Description**: Write unit tests using `tempdir` (or `std::env::temp_dir` + unique subdir) for isolation. Tests use real filesystem (not mocked) to verify atomic write behavior.
- **Acceptance Criteria**:
  - Test: load missing file → creates defaults with correct values
  - Test: save → load round-trip → values match
  - Test: load corrupt file → backup created, defaults returned
  - Test: atomic write verified (tmp file cleaned up, final file exists)
  - Test: schema version migration (simulate v0 → v1 upgrade path)
  - `cargo test -p oh-my-vpn -- preferences_store` passes (no `#[ignore]` -- these are pure filesystem tests, no OS integration needed)

---

## 4. Execution Strategy

| Step | Chain | Complexity | Rationale |
| --- | --- | --- | --- |
| 1 | scout → worker | Simple | Type definitions, clear spec from data model |
| 2 | scout → worker | Medium | Atomic write + corrupt recovery + migration logic |
| 3 | scout → worker → reviewer | Medium | Tests must cover all edge cases; reviewer verifies coverage |

**Execution order**: Step 1 → Step 2 → Step 3 (sequential -- each builds on previous)

**Parallel opportunities**: None -- strict dependency chain

**Risk flags**:

- Step 2 corrupt recovery: edge case where backup file already exists (overwrite previous backup)
- Step 3 test isolation: each test needs its own temp directory to avoid cross-contamination
