//! Cloud provider abstraction layer.
//!
//! Defines the `CloudProvider` trait and implements it for each supported
//! cloud provider (Hetzner, AWS, GCP). Handles API authentication,
//! region listing, and pricing queries.

mod aws;
mod cache;
mod cloud_provider;
mod gcp;
mod hetzner;
mod registry;

pub use aws::AwsProvider;
pub use cache::PricingCache;
pub use cloud_provider::CloudProvider;
pub use gcp::GcpProvider;
pub use hetzner::HetznerProvider;
pub use registry::ProviderRegistry;
