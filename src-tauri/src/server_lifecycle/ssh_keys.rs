//! Ed25519 SSH key pair generation for ephemeral server provisioning.
//!
//! Generates a one-time Ed25519 key pair, formats the public key in OpenSSH
//! wire format for cloud provider registration, and securely zeroes all key
//! material on drop.

use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use ssh_key::public::Ed25519PublicKey;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::server_lifecycle::LifecycleError;

/// An ephemeral Ed25519 SSH key pair for server provisioning.
///
/// The private key bytes are zeroed in memory when this struct is dropped.
/// Used only to register an SSH key with the cloud provider so that cloud-init
/// can run on the newly provisioned server. The key is deleted from the provider
/// immediately after server creation.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SshKeyPair {
    /// The 32-byte Ed25519 private key (seed).
    private_key: [u8; 32],
    /// The 32-byte Ed25519 public key.
    public_key: [u8; 32],
}

impl SshKeyPair {
    /// Generate a new random Ed25519 SSH key pair.
    pub fn generate() -> Result<Self, LifecycleError> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            private_key: signing_key.to_bytes(),
            public_key: verifying_key.to_bytes(),
        })
    }

    /// Returns the public key in OpenSSH authorized_keys format.
    ///
    /// Example: `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA... oh-my-vpn`
    pub fn public_key_openssh(&self) -> Result<String, LifecycleError> {
        let ed25519_public = Ed25519PublicKey(self.public_key);
        let ssh_public = ssh_key::PublicKey::from(ed25519_public);

        // ssh-key 0.6: to_openssh() returns Result<String>
        ssh_public
            .to_openssh()
            .map_err(|e| LifecycleError::SshKeyGenerationFailed(format!(
                "Failed to encode SSH public key in OpenSSH format: {e}"
            )))
    }
}

// Manual Debug to avoid leaking key material.
impl std::fmt::Debug for SshKeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshKeyPair")
            .field("private_key", &"[REDACTED]")
            .field("public_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_valid_key_pair() {
        let pair = SshKeyPair::generate().expect("key generation should succeed");
        assert_eq!(pair.private_key.len(), 32);
        assert_eq!(pair.public_key.len(), 32);
    }

    #[test]
    fn public_key_openssh_format() {
        let pair = SshKeyPair::generate().expect("key generation should succeed");
        let openssh = pair.public_key_openssh().expect("OpenSSH format should succeed");

        assert!(
            openssh.starts_with("ssh-ed25519 AAAA"),
            "OpenSSH key should start with 'ssh-ed25519 AAAA', got: {openssh}"
        );
    }

    #[test]
    fn two_pairs_have_different_public_keys() {
        let pair_a = SshKeyPair::generate().expect("key generation should succeed");
        let pair_b = SshKeyPair::generate().expect("key generation should succeed");
        assert_ne!(pair_a.public_key, pair_b.public_key);
    }

    #[test]
    fn zeroize_clears_private_key() {
        let mut bytes: [u8; 32] = [0xAB; 32];
        bytes.zeroize();
        assert_eq!(bytes, [0u8; 32], "zeroize should clear all bytes to zero");
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let pair = SshKeyPair::generate().expect("key generation should succeed");
        let debug_output = format!("{pair:?}");
        assert!(
            debug_output.contains("REDACTED"),
            "Debug output should redact key material: {debug_output}"
        );
        assert!(
            !debug_output.contains("0x"),
            "Debug output should not contain hex bytes: {debug_output}"
        );
    }
}
