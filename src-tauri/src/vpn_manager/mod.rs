//! WireGuard VPN tunnel management.
//!
//! Orchestrates wireguard-go and wg-quick to establish, monitor,
//! and tear down VPN tunnels. Manages ephemeral key generation
//! and configuration file lifecycle.

pub mod config;
pub mod keys;
pub mod tunnel;

pub use tunnel::tunnel_down_interface;
