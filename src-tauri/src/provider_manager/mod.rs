//! Cloud provider abstraction layer.
//!
//! Defines the `CloudProvider` trait and implements it for each supported
//! cloud provider (Hetzner, AWS, GCP). Handles API authentication,
//! region listing, and pricing queries.

mod cache;
mod cloud_provider;
mod registry;

pub use cache::PricingCache;
pub use cloud_provider::CloudProvider;
pub use registry::ProviderRegistry;
