//! macOS Keychain integration via the Security Framework.
//!
//! Stores and retrieves cloud provider API credentials securely.
//! Wraps the macOS Security Framework APIs behind a Rust interface.

use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
use security_framework::passwords::{delete_generic_password, set_generic_password};
use std::fmt;

/// macOS Security Framework error code for "item not found" (-25300).
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

use crate::types::Provider;

// ---- KeychainError ----

/// Errors that can arise when interacting with the macOS Keychain.
#[derive(Debug)]
pub enum KeychainError {
    /// The calling process was denied access to the Keychain item.
    AccessDenied(String),
    /// A write (add or update) operation failed.
    WriteFailed(String),
    /// The requested credential was not found (used for delete).
    NotFound(String),
    /// A search query failed for an unexpected reason.
    SearchFailed(String),
}

impl fmt::Display for KeychainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeychainError::AccessDenied(msg) => write!(f, "Keychain access denied: {msg}"),
            KeychainError::WriteFailed(msg) => write!(f, "Keychain write failed: {msg}"),
            KeychainError::NotFound(msg) => write!(f, "Keychain entry not found: {msg}"),
            KeychainError::SearchFailed(msg) => write!(f, "Keychain search failed: {msg}"),
        }
    }
}

impl std::error::Error for KeychainError {}

// ---- Credential ----

/// A resolved cloud-provider credential retrieved from the macOS Keychain.
#[derive(Debug, Clone)]
pub struct Credential {
    /// The cloud provider this credential belongs to.
    pub provider: Provider,
    /// The user-defined label stored in the Keychain account field.
    pub account_label: String,
    /// The raw API key, decrypted from the Keychain.
    pub api_key: String,
}

// ---- KeychainAdapter ----

/// Stateless facade over macOS Keychain operations for provider credentials.
///
/// Each provider maps to a single Keychain service (`"oh-my-vpn.{provider}"`)
/// and may have one associated account entry. The account field stores the
/// user-supplied `account_label` (e.g. an email address or key name).
pub struct KeychainAdapter;

impl KeychainAdapter {
    /// Store a provider API key in the macOS Keychain.
    ///
    /// Creates a new entry or silently updates the existing one if the same
    /// service + account pair already exists.
    ///
    /// - `service`: derived from `provider.service_name()` (e.g. `"oh-my-vpn.hetzner"`)
    /// - `account`: the `account_label` supplied by the caller
    /// - `password`: the raw `api_key` bytes
    pub fn store_credential(
        provider: &Provider,
        account_label: &str,
        api_key: &str,
    ) -> Result<(), KeychainError> {
        let service = provider.service_name();
        set_generic_password(&service, account_label, api_key.as_bytes())
            .map_err(|e| KeychainError::WriteFailed(e.to_string()))
    }

    /// Retrieve the stored credential for a provider.
    ///
    /// Searches by service name only -- the caller need not know the account
    /// label in advance. Returns `Ok(None)` when no entry exists.
    pub fn retrieve_credential(provider: &Provider) -> Result<Option<Credential>, KeychainError> {
        let service = provider.service_name();

        let results = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(&service)
            .load_attributes(true)
            .load_data(true)
            .limit(Limit::Max(1))
            .search();

        let results = match results {
            Ok(r) => r,
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(None),
            Err(e) => return Err(KeychainError::SearchFailed(e.to_string())),
        };

        for result in &results {
            if let SearchResult::Dict(_) = result {
                let map = result
                    .simplify_dict()
                    .ok_or_else(|| KeychainError::SearchFailed("failed to read attributes".to_string()))?;

                let account_label = map
                    .get("acct")
                    .cloned()
                    .ok_or_else(|| KeychainError::SearchFailed("missing account field in keychain entry".to_string()))?;

                let api_key = map
                    .get("v_Data")
                    .cloned()
                    .ok_or_else(|| KeychainError::SearchFailed("missing data field in keychain entry".to_string()))?;

                return Ok(Some(Credential {
                    provider: provider.clone(),
                    account_label,
                    api_key,
                }));
            }
        }

        Ok(None)
    }

    /// Delete the stored credential for a provider.
    ///
    /// Retrieves the entry first to obtain the account label (required by the
    /// Security Framework delete API), then removes it. Returns
    /// `KeychainError::NotFound` when no entry exists.
    pub fn delete_credential(provider: &Provider) -> Result<(), KeychainError> {
        let credential = Self::retrieve_credential(provider)?;

        let credential = credential.ok_or_else(|| {
            KeychainError::NotFound(format!(
                "no Keychain entry found for provider {}",
                provider
            ))
        })?;

        let service = provider.service_name();
        delete_generic_password(&service, &credential.account_label)
            .map_err(|e| KeychainError::NotFound(e.to_string()))
    }

    /// List all registered provider credentials.
    ///
    /// Searches all generic passwords, keeps only those whose service name
    /// starts with `"oh-my-vpn."`, and returns `(provider, account_label)`
    /// pairs. The API key itself is never included in the result.
    pub fn list_credentials() -> Result<Vec<(Provider, String)>, KeychainError> {
        let results = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .load_attributes(true)
            .load_data(false)
            .limit(Limit::All)
            .search();

        let results = match results {
            Ok(r) => r,
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(vec![]),
            Err(e) => return Err(KeychainError::SearchFailed(e.to_string())),
        };

        let mut credentials = Vec::new();

        for result in &results {
            if let SearchResult::Dict(_) = result {
                let map = match result.simplify_dict() {
                    Some(m) => m,
                    None => continue,
                };

                let service = match map.get("svce") {
                    Some(s) if s.starts_with("oh-my-vpn.") => s.clone(),
                    _ => continue,
                };

                let provider = match Provider::from_service_name(&service) {
                    Some(p) => p,
                    None => continue,
                };

                let account_label = match map.get("acct").cloned() {
                    Some(a) => a,
                    None => continue,
                };

                credentials.push((provider, account_label));
            }
        }

        Ok(credentials)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shared test account label -- avoids collisions with real Keychain entries.
    const TEST_LABEL: &str = "test-oh-my-vpn-integration";

    /// Best-effort cleanup helper. Silently ignores NotFound so it is safe to
    /// call even when no entry exists (e.g. at the start of a test to clear
    /// any leftover state from a previous interrupted run).
    fn cleanup(provider: &Provider) {
        let _ = KeychainAdapter::delete_credential(provider);
    }

    // ------------------------------------------------------------------ //
    // 1. Round-trip: store → retrieve → verify → delete → verify None     //
    // ------------------------------------------------------------------ //

    #[test]
    #[ignore]
    fn test_round_trip() {
        let provider = Provider::Hetzner;
        let api_key = "test-api-key-round-trip";

        // Guard: remove any leftover from a previous run.
        cleanup(&provider);

        // Store.
        KeychainAdapter::store_credential(&provider, TEST_LABEL, api_key)
            .expect("store_credential should succeed");

        // Retrieve and verify content.
        let credential = KeychainAdapter::retrieve_credential(&provider)
            .expect("retrieve_credential should not error")
            .expect("credential should exist after store");

        assert_eq!(credential.provider, provider);
        assert_eq!(credential.account_label, TEST_LABEL);
        assert_eq!(credential.api_key, api_key);

        // Delete.
        KeychainAdapter::delete_credential(&provider)
            .expect("delete_credential should succeed");

        // Verify gone.
        let result = KeychainAdapter::retrieve_credential(&provider)
            .expect("retrieve_credential should not error after delete");
        assert!(result.is_none(), "credential should be absent after delete");
    }

    // ------------------------------------------------------------------ //
    // 2. Retrieve non-existent: expect Ok(None)                           //
    // ------------------------------------------------------------------ //

    #[test]
    #[ignore]
    fn test_retrieve_non_existent() {
        let provider = Provider::Aws;

        // Guard: ensure no entry exists before the test.
        cleanup(&provider);

        let result = KeychainAdapter::retrieve_credential(&provider)
            .expect("retrieve_credential should not error for missing entry");

        assert!(
            result.is_none(),
            "expected Ok(None) for a provider with no stored credential"
        );
    }

    // ------------------------------------------------------------------ //
    // 3. Delete non-existent: expect KeychainError::NotFound              //
    // ------------------------------------------------------------------ //

    #[test]
    #[ignore]
    fn test_delete_non_existent() {
        let provider = Provider::Gcp;

        // Guard: ensure no entry exists before the test.
        cleanup(&provider);

        let result = KeychainAdapter::delete_credential(&provider);

        assert!(
            matches!(result, Err(KeychainError::NotFound(_))),
            "expected KeychainError::NotFound when deleting a non-existent entry, got: {result:?}"
        );
    }

    // ------------------------------------------------------------------ //
    // 4. List: store 2 providers → list → verify both → clean up          //
    // ------------------------------------------------------------------ //

    #[test]
    #[ignore]
    fn test_list_credentials() {
        let provider_a = Provider::Hetzner;
        let provider_b = Provider::Aws;

        // Guard: remove any leftover from a previous run.
        cleanup(&provider_a);
        cleanup(&provider_b);

        // Store two distinct credentials.
        KeychainAdapter::store_credential(&provider_a, TEST_LABEL, "key-a")
            .expect("store provider_a should succeed");
        KeychainAdapter::store_credential(&provider_b, TEST_LABEL, "key-b")
            .expect("store provider_b should succeed");

        // List and verify.
        let list = KeychainAdapter::list_credentials()
            .expect("list_credentials should not error");

        let has_a = list
            .iter()
            .any(|(p, lbl)| p == &provider_a && lbl == TEST_LABEL);
        let has_b = list
            .iter()
            .any(|(p, lbl)| p == &provider_b && lbl == TEST_LABEL);

        // Clean up before assertions so the Keychain is always left clean.
        cleanup(&provider_a);
        cleanup(&provider_b);

        assert!(has_a, "list should include the Hetzner test credential");
        assert!(has_b, "list should include the AWS test credential");
    }

    // ------------------------------------------------------------------ //
    // 5. Upsert: store → store again (different key) → verify new value   //
    // ------------------------------------------------------------------ //

    #[test]
    #[ignore]
    fn test_upsert_credential() {
        let provider = Provider::Gcp;
        let original_key = "original-api-key";
        let updated_key = "updated-api-key";

        // Guard: remove any leftover from a previous run.
        cleanup(&provider);

        // First store.
        KeychainAdapter::store_credential(&provider, TEST_LABEL, original_key)
            .expect("first store should succeed");

        // Second store (same service + account, different key -- should upsert).
        KeychainAdapter::store_credential(&provider, TEST_LABEL, updated_key)
            .expect("second store (upsert) should succeed");

        // Retrieve and verify the updated value.
        let credential = KeychainAdapter::retrieve_credential(&provider)
            .expect("retrieve_credential should not error")
            .expect("credential should exist after upsert");

        // Clean up before assertions.
        cleanup(&provider);

        assert_eq!(
            credential.api_key, updated_key,
            "retrieved key should match the updated value, not the original"
        );
        assert_eq!(credential.account_label, TEST_LABEL);
        assert_eq!(credential.provider, provider);
    }
}
