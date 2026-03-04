//! Server provisioning and destruction lifecycle.
//!
//! Coordinates the full server lifecycle: create VM, configure
//! WireGuard, establish tunnel, and destroy on disconnect.
//! Implements the stepper flow (provision → configure → connect).
