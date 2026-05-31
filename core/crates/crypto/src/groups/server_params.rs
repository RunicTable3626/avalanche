//! Per-homeserver zkgroup signing keys.
//!
//! Each homeserver holds exactly one [`ServerSecretParams`] — the signing key
//! for anonymous credentials and group send endorsements. The matching
//! [`ServerPublicParams`] is published so clients can verify issuances and
//! produce presentations. Persistence (DB storage, hot-reload on startup) is
//! the server's job; this module only provides the crypto-level primitives.
//!
//! Both types are thin wrappers around the libsignal `zkgroup` equivalents.
//! Serialization uses zkgroup's own `bincode`-based codec so the bytes round-
//! trip across versions of this codebase that share a pinned libsignal commit.

use rand::TryRngCore;

use crate::error::CryptoError;

/// Server-side signing key. Generated once at first boot, then stored.
/// Holding this is equivalent to holding the homeserver's authority to issue
/// auth credentials and group send endorsements.
#[derive(Clone)]
pub struct ServerSecretParams(zkgroup::ServerSecretParams);

/// Public counterpart published to clients so they can verify credential
/// issuances and produce presentations.
#[derive(Clone)]
pub struct ServerPublicParams(zkgroup::ServerPublicParams);

impl ServerSecretParams {
    /// Generate a fresh keypair using the OS RNG.
    pub fn generate() -> Self {
        let mut randomness = [0u8; zkgroup::RANDOMNESS_LEN];
        // OsRng failure here means the OS RNG itself is broken; we can't
        // recover and shouldn't pretend to.
        rand::rngs::OsRng
            .try_fill_bytes(&mut randomness)
            .expect("OS RNG failed");
        Self(zkgroup::ServerSecretParams::generate(randomness))
    }

    /// Derive the matching public params.
    pub fn public_params(&self) -> ServerPublicParams {
        ServerPublicParams(self.0.get_public_params())
    }

    /// Encode for at-rest storage (homeserver DB). Stable for a given
    /// pinned libsignal commit.
    pub fn to_bytes(&self) -> Vec<u8> {
        zkgroup::serialize(&self.0)
    }

    /// Decode from at-rest storage.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        zkgroup::deserialize::<zkgroup::ServerSecretParams>(bytes)
            .map(Self)
            .map_err(|_| CryptoError::ZkgroupDeserialize)
    }

    /// Borrow the underlying libsignal type. Crate-internal callers (e.g.
    /// credential issuance) need this; external callers should go through the
    /// scheme-agnostic API in higher-level modules.
    #[allow(dead_code)] // used by upcoming credentials module
    pub(crate) fn inner(&self) -> &zkgroup::ServerSecretParams {
        &self.0
    }
}

impl ServerPublicParams {
    /// Encode for transport over the wire (clients fetch this).
    pub fn to_bytes(&self) -> Vec<u8> {
        zkgroup::serialize(&self.0)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        zkgroup::deserialize::<zkgroup::ServerPublicParams>(bytes)
            .map(Self)
            .map_err(|_| CryptoError::ZkgroupDeserialize)
    }

    #[allow(dead_code)] // used by upcoming credentials module
    pub(crate) fn inner(&self) -> &zkgroup::ServerPublicParams {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_roundtrip_secret() {
        let secret = ServerSecretParams::generate();
        let bytes = secret.to_bytes();
        let decoded = ServerSecretParams::from_bytes(&bytes).expect("decode");
        // Re-encoding the decoded value yields identical bytes.
        assert_eq!(bytes, decoded.to_bytes());
    }

    #[test]
    fn public_params_derive_and_roundtrip() {
        let secret = ServerSecretParams::generate();
        let public = secret.public_params();
        let bytes = public.to_bytes();
        let decoded = ServerPublicParams::from_bytes(&bytes).expect("decode");
        assert_eq!(bytes, decoded.to_bytes());
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        assert!(ServerSecretParams::from_bytes(&[0u8; 8]).is_err());
        assert!(ServerPublicParams::from_bytes(&[0u8; 8]).is_err());
    }

    #[test]
    fn distinct_generates_differ() {
        let a = ServerSecretParams::generate();
        let b = ServerSecretParams::generate();
        assert_ne!(a.to_bytes(), b.to_bytes());
    }

}
