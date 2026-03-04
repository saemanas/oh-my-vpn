//! WireGuard Curve25519 key pair generation.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand_core::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// An ephemeral Curve25519 key pair used for WireGuard tunnels.
///
/// The private key bytes are zeroed in memory when this struct is dropped.
#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct WireGuardKeyPair {
    /// The 32-byte Curve25519 private key.
    pub private_key: [u8; 32],
    /// The 32-byte Curve25519 public key derived from the private key.
    pub public_key: [u8; 32],
}

impl WireGuardKeyPair {
    /// Generate a new random WireGuard key pair using a cryptographically secure OS RNG.
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self {
            private_key: secret.to_bytes(),
            public_key: *public.as_bytes(),
        }
    }

    /// Returns the public key encoded as a standard Base64 string (44 characters).
    pub fn public_key_base64(&self) -> String {
        STANDARD.encode(self.public_key)
    }

    /// Returns the private key encoded as a standard Base64 string (44 characters).
    pub fn private_key_base64(&self) -> String {
        STANDARD.encode(self.private_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    #[test]
    fn generate_produces_32_byte_keys() {
        let pair = WireGuardKeyPair::generate();
        assert_eq!(pair.private_key.len(), 32);
        assert_eq!(pair.public_key.len(), 32);
    }

    #[test]
    fn base64_output_is_44_chars() {
        let pair = WireGuardKeyPair::generate();
        assert_eq!(pair.public_key_base64().len(), 44);
        assert_eq!(pair.private_key_base64().len(), 44);
    }

    #[test]
    fn two_pairs_have_different_public_keys() {
        let pair_a = WireGuardKeyPair::generate();
        let pair_b = WireGuardKeyPair::generate();
        assert_ne!(pair_a.public_key, pair_b.public_key);
    }

    #[test]
    fn zeroize_clears_bytes() {
        let mut bytes: [u8; 32] = [0xAB; 32];
        bytes.zeroize();
        assert_eq!(bytes, [0u8; 32]);
    }
}
