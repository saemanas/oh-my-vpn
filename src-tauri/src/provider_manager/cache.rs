//! In-memory pricing cache with TTL-based expiration and stale fallback.
//!
//! See Data Model §4.C for the PricingCache schema.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::types::{Provider, RegionInfo};

/// Default TTL for cached pricing data (1 hour).
const DEFAULT_TTL_SECONDS: u64 = 3600;

/// A single cache entry holding region pricing data with timestamp.
struct CacheEntry {
    regions: Vec<RegionInfo>,
    fetched_at: Instant,
}

/// In-memory cache for provider region/pricing data.
///
/// Provides TTL-based expiration with a stale fallback mechanism:
/// - `get()` returns data only if within TTL
/// - `get_stale()` returns data regardless of TTL (for use when API fails)
pub struct PricingCache {
    entries: HashMap<Provider, CacheEntry>,
    ttl: Duration,
}

impl PricingCache {
    /// Create a new cache with the default TTL (3600 seconds).
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            ttl: Duration::from_secs(DEFAULT_TTL_SECONDS),
        }
    }

    /// Create a new cache with a custom TTL.
    #[cfg(test)]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    /// Store region pricing data for a provider, resetting the TTL timer.
    pub fn set(&mut self, provider: Provider, regions: Vec<RegionInfo>) {
        self.entries.insert(
            provider,
            CacheEntry {
                regions,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Retrieve cached data if the entry exists and is within TTL.
    /// Returns `None` if expired or absent.
    pub fn get(&self, provider: &Provider) -> Option<&[RegionInfo]> {
        self.entries.get(provider).and_then(|entry| {
            if entry.fetched_at.elapsed() < self.ttl {
                Some(entry.regions.as_slice())
            } else {
                None
            }
        })
    }

    /// Retrieve cached data regardless of TTL. Used as a stale fallback
    /// when the provider API is unavailable.
    pub fn get_stale(&self, provider: &Provider) -> Option<&[RegionInfo]> {
        self.entries
            .get(provider)
            .map(|entry| entry.regions.as_slice())
    }

    /// Remove cached data for a specific provider.
    pub fn invalidate(&mut self, provider: &Provider) {
        self.entries.remove(provider);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn sample_regions() -> Vec<RegionInfo> {
        vec![
            RegionInfo {
                region: "fsn1".to_string(),
                display_name: "Falkenstein, DE".to_string(),
                instance_type: "cx22".to_string(),
                hourly_cost: 0.0065,
            },
            RegionInfo {
                region: "nbg1".to_string(),
                display_name: "Nuremberg, DE".to_string(),
                instance_type: "cx22".to_string(),
                hourly_cost: 0.0065,
            },
        ]
    }

    #[test]
    fn set_then_get_returns_fresh_data() {
        let mut cache = PricingCache::new();
        cache.set(Provider::Hetzner, sample_regions());

        let result = cache.get(&Provider::Hetzner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn get_returns_none_for_absent_provider() {
        let cache = PricingCache::new();
        assert!(cache.get(&Provider::Aws).is_none());
    }

    #[test]
    fn get_returns_none_after_ttl_expires() {
        let mut cache = PricingCache::with_ttl(Duration::from_millis(1));
        cache.set(Provider::Hetzner, sample_regions());

        // Wait for TTL to expire.
        thread::sleep(Duration::from_millis(5));

        assert!(cache.get(&Provider::Hetzner).is_none());
    }

    #[test]
    fn get_stale_returns_data_after_ttl_expires() {
        let mut cache = PricingCache::with_ttl(Duration::from_millis(1));
        cache.set(Provider::Hetzner, sample_regions());

        thread::sleep(Duration::from_millis(5));

        let result = cache.get_stale(&Provider::Hetzner);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn get_stale_returns_none_for_absent_provider() {
        let cache = PricingCache::new();
        assert!(cache.get_stale(&Provider::Aws).is_none());
    }

    #[test]
    fn invalidate_removes_entry() {
        let mut cache = PricingCache::new();
        cache.set(Provider::Hetzner, sample_regions());
        cache.invalidate(&Provider::Hetzner);

        assert!(cache.get(&Provider::Hetzner).is_none());
        assert!(cache.get_stale(&Provider::Hetzner).is_none());
    }

    #[test]
    fn providers_are_independent() {
        let mut cache = PricingCache::new();
        cache.set(Provider::Hetzner, sample_regions());

        assert!(cache.get(&Provider::Hetzner).is_some());
        assert!(cache.get(&Provider::Aws).is_none());
        assert!(cache.get(&Provider::Gcp).is_none());
    }
}
