//! User preferences persistence via atomic JSON file operations.
//!
//! Manages user settings such as default provider, preferred region,
//! notification preferences, and keyboard shortcuts. Stored as a JSON
//! file in the Tauri app data directory with crash-safe atomic writes.

use std::fmt;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::Provider;

const CURRENT_SCHEMA_VERSION: u32 = 1;

// -- Error

#[derive(Debug)]
pub enum PreferencesError {
    ReadFailed(String),
    WriteFailed(String),
    ParseFailed(String),
    MigrationFailed(String),
}

impl fmt::Display for PreferencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreferencesError::ReadFailed(msg) => write!(f, "Preferences read failed: {msg}"),
            PreferencesError::WriteFailed(msg) => write!(f, "Preferences write failed: {msg}"),
            PreferencesError::ParseFailed(msg) => write!(f, "Preferences parse failed: {msg}"),
            PreferencesError::MigrationFailed(msg) => {
                write!(f, "Preferences migration failed: {msg}")
            }
        }
    }
}

impl std::error::Error for PreferencesError {}

// -- UserPreferences

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    pub schema_version: u32,
    pub last_provider: Option<Provider>,
    pub last_region: Option<String>,
    pub notifications_enabled: bool,
    pub keyboard_shortcut: Option<String>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_provider: None,
            last_region: None,
            notifications_enabled: true,
            keyboard_shortcut: None,
        }
    }
}

// -- PreferencesStore

pub struct PreferencesStore {
    data_dir: PathBuf,
}

impl PreferencesStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn file_path(&self) -> PathBuf {
        self.data_dir.join("preferences.json")
    }

    pub fn save(&self, preferences: &UserPreferences) -> Result<(), PreferencesError> {
        fs::create_dir_all(&self.data_dir).map_err(|e| {
            PreferencesError::WriteFailed(format!("Failed to create data directory: {e}"))
        })?;

        let json = serde_json::to_string_pretty(preferences).map_err(|e| {
            PreferencesError::WriteFailed(format!("Failed to serialize preferences: {e}"))
        })?;

        let tmp_path = self.data_dir.join(".preferences.tmp.json");

        fs::write(&tmp_path, &json).map_err(|e| {
            PreferencesError::WriteFailed(format!("Failed to write temp file: {e}"))
        })?;

        fs::rename(&tmp_path, self.file_path()).map_err(|e| {
            PreferencesError::WriteFailed(format!("Failed to atomically rename preferences file: {e}"))
        })?;

        Ok(())
    }

    pub fn load(&self) -> Result<UserPreferences, PreferencesError> {
        let file_path = self.file_path();

        if !file_path.exists() {
            let defaults = UserPreferences::default();
            self.save(&defaults)?;
            return Ok(defaults);
        }

        let content = fs::read_to_string(&file_path).map_err(|e| {
            PreferencesError::ReadFailed(format!("Failed to read preferences file: {e}"))
        })?;

        match serde_json::from_str::<UserPreferences>(&content) {
            Ok(preferences) => {
                if preferences.schema_version < CURRENT_SCHEMA_VERSION {
                    let migrated = Self::migrate(preferences)?;
                    self.save(&migrated)?;
                    Ok(migrated)
                } else {
                    Ok(preferences)
                }
            }
            Err(_parse_err) => {
                // Corrupt file -- back up and recreate with defaults
                let backup_path = self.data_dir.join("preferences.backup.json");
                let _ = fs::rename(&file_path, &backup_path);

                let defaults = UserPreferences::default();
                self.save(&defaults)?;
                Ok(defaults)
            }
        }
    }

    fn migrate(preferences: UserPreferences) -> Result<UserPreferences, PreferencesError> {
        let mut current = preferences;

        // Sequential migration: apply each step in order until we reach current version.
        // Future migrations: add a new match arm for the next version.
        loop {
            match current.schema_version {
                v if v == CURRENT_SCHEMA_VERSION => return Ok(current),
                0 => {
                    return Err(PreferencesError::MigrationFailed(
                        "No migration path from schema version 0".to_string(),
                    ));
                }
                v => {
                    return Err(PreferencesError::MigrationFailed(format!(
                        "Unknown schema version: {v}"
                    )));
                }
            }
        }
    }
}

// -- Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a PreferencesStore backed by a unique temp directory.
    /// Cleans up any leftovers from previous runs, but does NOT create the directory --
    /// some tests verify that save() creates it on demand.
    fn test_store(name: &str) -> (PreferencesStore, PathBuf) {
        let dir = std::env::temp_dir()
            .join("oh-my-vpn-prefs-test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        let store = PreferencesStore::new(dir.clone());
        (store, dir)
    }

    #[test]
    fn test_load_missing_file_creates_defaults() {
        let (store, _dir) = test_store("missing_file");

        let prefs = store.load().expect("load should succeed on missing file");

        assert_eq!(prefs.schema_version, 1);
        assert!(prefs.notifications_enabled);
        assert!(prefs.last_provider.is_none());
        assert!(prefs.last_region.is_none());
        assert!(prefs.keyboard_shortcut.is_none());
        assert!(store.file_path().exists(), "preferences.json should be created");
    }

    #[test]
    fn test_save_load_round_trip() {
        let (store, _dir) = test_store("round_trip");

        let prefs = UserPreferences {
            schema_version: 1,
            last_provider: Some(Provider::Hetzner),
            last_region: Some("eu-central".to_string()),
            notifications_enabled: false,
            keyboard_shortcut: Some("Cmd+Shift+V".to_string()),
        };

        store.save(&prefs).expect("save should succeed");
        let loaded = store.load().expect("load should succeed");

        assert_eq!(loaded, prefs);
    }

    #[test]
    fn test_load_corrupt_file_creates_backup() {
        let (store, dir) = test_store("corrupt_file");

        // Create the directory and write corrupt JSON
        fs::create_dir_all(&dir).expect("failed to create test dir");
        let corrupt_content = "this is not valid json {{{";
        fs::write(store.file_path(), corrupt_content).expect("failed to write corrupt file");

        let result = store.load().expect("load should return Ok(defaults) for a corrupt file");

        // Returns defaults
        assert_eq!(result, UserPreferences::default());

        // Backup exists and contains the original corrupt content
        let backup_path = dir.join("preferences.backup.json");
        assert!(backup_path.exists(), "preferences.backup.json should exist");
        let backup_content =
            fs::read_to_string(&backup_path).expect("failed to read backup file");
        assert_eq!(backup_content, corrupt_content);

        // preferences.json now contains valid defaults
        let new_content =
            fs::read_to_string(store.file_path()).expect("failed to read preferences.json");
        let new_prefs: UserPreferences =
            serde_json::from_str(&new_content).expect("preferences.json should be valid JSON");
        assert_eq!(new_prefs, UserPreferences::default());
    }

    #[test]
    fn test_atomic_write_no_tmp_leftover() {
        let (store, dir) = test_store("atomic_write");

        store.save(&UserPreferences::default()).expect("save should succeed");

        // Temp file must not survive the atomic rename
        let tmp_path = dir.join(".preferences.tmp.json");
        assert!(!tmp_path.exists(), ".preferences.tmp.json should not exist after save");

        // Final file must exist and parse correctly
        assert!(store.file_path().exists(), "preferences.json should exist");
        let content =
            fs::read_to_string(store.file_path()).expect("failed to read preferences.json");
        let loaded: UserPreferences =
            serde_json::from_str(&content).expect("preferences.json should be valid JSON");
        assert_eq!(loaded, UserPreferences::default());
    }

    #[test]
    fn test_schema_migration() {
        let (store, dir) = test_store("schema_migration");

        // Write a structurally valid JSON file with an old schema_version=0
        fs::create_dir_all(&dir).expect("failed to create test dir");
        let old_json = r#"{
  "schemaVersion": 0,
  "lastProvider": null,
  "lastRegion": null,
  "notificationsEnabled": true,
  "keyboardShortcut": null
}"#;
        fs::write(store.file_path(), old_json).expect("failed to write old-schema file");

        let result = store.load();

        assert!(
            matches!(result, Err(PreferencesError::MigrationFailed(_))),
            "expected MigrationFailed for schema_version=0, got: {:?}",
            result
        );
    }

    #[test]
    fn test_save_creates_directory() {
        // Use a nested path that does not exist yet
        let dir = std::env::temp_dir()
            .join("oh-my-vpn-prefs-test")
            .join("save_creates_directory")
            .join("nested");
        let _ = fs::remove_dir_all(&dir);

        let store = PreferencesStore::new(dir.clone());

        assert!(!dir.exists(), "directory should not exist before save");
        store
            .save(&UserPreferences::default())
            .expect("save should create the directory and succeed");

        assert!(dir.exists(), "directory should be created by save");
        assert!(store.file_path().exists(), "preferences.json should exist");
    }
}
