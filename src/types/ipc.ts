/**
 * IPC type definitions mirroring the Rust backend.
 *
 * Field names are camelCase to match `#[serde(rename_all = "camelCase")]`
 * on the Rust side. Enum variants are lowercase string literals to match
 * `#[serde(rename_all = "lowercase")]`.
 *
 * Sources:
 *   src-tauri/src/types.rs
 *   src-tauri/src/session_tracker.rs
 */

// ── Enum types ─────────────────────────────────────────────────────────────

/** Cloud provider. Mirrors Rust `Provider` enum (serde: lowercase). */
export type Provider = "hetzner" | "aws" | "gcp";

/**
 * Provider credential validation status.
 * Mirrors Rust `ProviderStatus` enum (serde: lowercase).
 */
export type ProviderStatus = "valid" | "invalid" | "unchecked";

/** Server lifecycle status. Mirrors Rust `ServerStatus` enum (serde: lowercase). */
export type ServerStatus = "provisioning" | "running" | "deleting";

/**
 * Action to take on an orphaned server.
 * Mirrors Rust `OrphanAction` enum (serde: lowercase).
 */
export type OrphanAction = "destroy" | "reconnect";

// ── Interfaces ─────────────────────────────────────────────────────────────

/**
 * Provider information for UI display.
 * Mirrors Rust `ProviderInfo` struct (serde: camelCase).
 */
export interface ProviderInfo {
  provider: Provider;
  status: ProviderStatus;
  /** Human-readable identifier from the Keychain account field. */
  accountLabel: string;
}

/**
 * Cloud region with pricing information.
 * Mirrors Rust `RegionInfo` struct (serde: camelCase).
 */
export interface RegionInfo {
  /** Cloud region code, e.g. "fsn1", "us-east-1". */
  region: string;
  /** Human-readable region name, e.g. "Falkenstein, DE". */
  displayName: string;
  /** Cheapest available instance type, e.g. "cx22". */
  instanceType: string;
  /** Hourly cost in USD. */
  hourlyCost: number;
}

/**
 * Server information returned by cloud provider operations.
 * Mirrors Rust `ServerInfo` struct (serde: camelCase).
 */
export interface ServerInfo {
  serverId: string;
  publicIp: string;
  status: ServerStatus;
}

/**
 * Active session status for the connected view.
 * Mirrors Rust `SessionStatus` struct (serde: camelCase).
 * Source: src-tauri/src/session_tracker.rs
 */
export interface SessionStatus {
  provider: Provider;
  /** Cloud region code, e.g. "fsn1". */
  region: string;
  /** Human-readable region name, e.g. "Falkenstein, DE". */
  regionDisplayName: string;
  /** Public IP address of the VPN server. */
  serverIp: string;
  /** Seconds elapsed since the session started. */
  elapsedSeconds: number;
  /** Hourly cost in USD (captured at session creation). */
  hourlyCost: number;
  /** Accumulated cost in USD since the session started. */
  accumulatedCost: number;
}

/**
 * User preferences stored locally.
 * Mirrors Rust `UserPreferences` (serde: camelCase).
 * Source: docs/api-design §4.E (get_preferences)
 */
export interface UserPreferences {
  lastProvider: Provider | null;
  lastRegion: string | null;
  notificationsEnabled: boolean;
  keyboardShortcut: string | null;
}

/**
 * An orphaned server detected on app launch.
 * Mirrors Rust `OrphanedServer` struct (serde: camelCase).
 */
export interface OrphanedServer {
  serverId: string;
  provider: Provider;
  /** Cloud region code, e.g. "fsn1". */
  region: string;
  /** ISO 8601 datetime when the server was originally created. */
  createdAt: string;
  /** Accumulated cost in USD since createdAt. */
  estimatedCost: number;
}
