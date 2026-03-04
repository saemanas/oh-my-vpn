//! Provider registry -- manages cloud provider instances and pricing cache.
//!
//! `ProviderRegistry` stores trait objects keyed by `Provider` enum,
//! enabling Dependency Inversion: business logic depends on the
//! `CloudProvider` trait, never on concrete provider implementations.

use std::collections::HashMap;

use crate::types::Provider;

use super::cache::PricingCache;
use super::cloud_provider::CloudProvider;

/// Central registry of cloud provider implementations.
///
/// Owns a `PricingCache` internally. When a provider is removed,
/// its cached pricing data is automatically invalidated.
///
/// API keys are **never** cached in the registry -- each operation
/// retrieves credentials from the Keychain via `KeychainAdapter`.
pub struct ProviderRegistry {
    providers: HashMap<Provider, Box<dyn CloudProvider>>,
    pricing_cache: PricingCache,
}

impl ProviderRegistry {
    /// Create an empty registry with a default pricing cache.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            pricing_cache: PricingCache::new(),
        }
    }

    /// Register a cloud provider implementation.
    /// Replaces any existing implementation for the same provider.
    pub fn register(&mut self, provider: Provider, implementation: Box<dyn CloudProvider>) {
        self.providers.insert(provider, implementation);
    }

    /// Retrieve a reference to a registered provider's trait object.
    /// Returns `None` if the provider is not registered.
    pub fn get(&self, provider: &Provider) -> Option<&dyn CloudProvider> {
        self.providers.get(provider).map(|boxed| boxed.as_ref())
    }

    /// Remove a provider and invalidate its pricing cache.
    pub fn remove(&mut self, provider: &Provider) {
        self.providers.remove(provider);
        self.pricing_cache.invalidate(provider);
    }

    /// List all registered providers.
    pub fn list(&self) -> Vec<Provider> {
        self.providers.keys().cloned().collect()
    }

    /// Immutable access to the pricing cache.
    pub fn cache(&self) -> &PricingCache {
        &self.pricing_cache
    }

    /// Mutable access to the pricing cache.
    pub fn cache_mut(&mut self) -> &mut PricingCache {
        &mut self.pricing_cache
    }
}
