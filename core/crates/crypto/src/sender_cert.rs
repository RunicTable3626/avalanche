//! Sender Certificate issuance and verification.
//!
//! Used by the sealed-sender envelope (see `crypto::sealed_sender`): each
//! group-message envelope embeds a [`SenderCertificate`] signed by the
//! homeserver, attesting "this identity key belongs to this DID + device,
//! valid until this time." Recipients verify the cert against a pinned
//! `trust_root` public key fetched from the server at startup; once
//! verified, they trust the embedded identity key for the duration of
//! the envelope's decryption.
//!
//! Cert chain shape:
//! ```text
//! trust_root (long-term, server-side)
//!   └─ ServerCertificate (key_id=1, signs sender certs; same keypair as trust_root for simplicity)
//!        └─ SenderCertificate (per-(did, day): sender_uuid, identity_key, expiration)
//! ```
//!
//! For Stage 5 we use a single-key chain: `trust_root == ServerCertificate
//! leaf key`. The two-level structure exists in libsignal so revocation
//! can shorten a compromised leaf key without rotating the trust root;
//! we don't exercise that yet, but the wire format supports it.

use libsignal_protocol::{
    PrivateKey, PublicKey, SenderCertificate, ServerCertificate, Timestamp,
};
use rand::rngs::OsRng;
use rand::TryRngCore as _;
use serde::{Deserialize, Serialize};

// (No bespoke randomness helper here: libsignal's `KeyPair::generate`,
// `ServerCertificate::new`, and `SenderCertificate::new` all take a
// `&mut R: Rng + CryptoRng` so we pass `OsRng.unwrap_err()` directly.)

use crate::error::CryptoError;

/// Server-side sender-certificate chain. Generated once at first boot,
/// persisted alongside the homeserver's zkgroup params, and used to
/// issue daily [`SenderCertificate`]s.
#[derive(Clone)]
pub struct SenderCertChain {
    trust_root_priv: PrivateKey,
    trust_root_pub: PublicKey,
    server_cert: ServerCertificate,
}

#[derive(Serialize, Deserialize)]
struct ChainWire {
    trust_root_priv: Vec<u8>,
    trust_root_pub: Vec<u8>,
    server_cert: Vec<u8>,
}

impl SenderCertChain {
    /// Generate a fresh chain. Single-key shape: trust_root and the
    /// ServerCertificate's leaf key are the same keypair, signed by
    /// itself with `key_id=1`.
    pub fn generate() -> Result<Self, CryptoError> {
        // libsignal-protocol uses Curve25519; KeyPair::generate is the
        // canonical constructor. We work with the (priv, pub) pair
        // directly because ServerCertificate needs both halves.
        let mut rng = OsRng.unwrap_err();
        let kp = libsignal_protocol::KeyPair::generate(&mut rng);
        let trust_root_priv = kp.private_key;
        let trust_root_pub = kp.public_key;

        let server_cert =
            ServerCertificate::new(1, trust_root_pub, &trust_root_priv, &mut rng)?;
        Ok(Self {
            trust_root_priv,
            trust_root_pub,
            server_cert,
        })
    }

    pub fn trust_root_public_bytes(&self) -> Vec<u8> {
        self.trust_root_pub.serialize().to_vec()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let wire = ChainWire {
            trust_root_priv: self.trust_root_priv.serialize().to_vec(),
            trust_root_pub: self.trust_root_pub.serialize().to_vec(),
            server_cert: self.server_cert.serialized().expect("serialize").to_vec(),
        };
        bincode::serialize(&wire).expect("serialize ChainWire")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let wire: ChainWire =
            bincode::deserialize(bytes).map_err(|_| CryptoError::ZkgroupDeserialize)?;
        let trust_root_priv = PrivateKey::deserialize(&wire.trust_root_priv)
            .map_err(|_| CryptoError::InvalidKey)?;
        let trust_root_pub = PublicKey::deserialize(&wire.trust_root_pub)
            .map_err(|_| CryptoError::InvalidKey)?;
        let server_cert = ServerCertificate::deserialize(&wire.server_cert)?;
        Ok(Self {
            trust_root_priv,
            trust_root_pub,
            server_cert,
        })
    }

    /// Issue a sender certificate. `sender_identity_key` is the
    /// sender's public Curve25519 identity key (33-byte form, as
    /// returned by `IdentityKey::public_key().serialize()`).
    ///
    /// `expiration_unix_millis` is millis-since-epoch; clients reject
    /// certs with `validation_time > expiration`.
    pub fn issue_sender_cert(
        &self,
        sender_did: &str,
        sender_device_id: u32,
        sender_identity_key: &[u8],
        expiration_unix_millis: u64,
    ) -> Result<Vec<u8>, CryptoError> {
        let identity_pub = PublicKey::deserialize(sender_identity_key)
            .map_err(|_| CryptoError::InvalidKey)?;
        let device_id: libsignal_protocol::DeviceId = sender_device_id
            .try_into()
            .map_err(|_| CryptoError::InvalidCiphertext)?;
        let mut rng = OsRng.unwrap_err();
        let cert = SenderCertificate::new(
            sender_did.to_string(),
            None,
            identity_pub,
            device_id,
            Timestamp::from_epoch_millis(expiration_unix_millis),
            self.server_cert.clone(),
            &self.trust_root_priv,
            &mut rng,
        )?;
        Ok(cert.serialized()?.to_vec())
    }

}

/// Decoded sender certificate, after [`validate_sender_cert`] confirms
/// the chain + expiration.
#[derive(Debug, Clone)]
pub struct SenderCertInfo {
    pub sender_did: String,
    pub sender_device_id: u32,
    pub identity_key_pub: Vec<u8>,
    pub expiration_unix_millis: u64,
}

/// Client-side: validate a serialized SenderCertificate against the
/// pinned trust-root public key. Returns the bound (DID, device,
/// identity-key) tuple if valid; errors otherwise.
pub fn validate_sender_cert(
    cert_bytes: &[u8],
    trust_root_pub: &[u8],
    validation_time_unix_millis: u64,
) -> Result<SenderCertInfo, CryptoError> {
    let cert = SenderCertificate::deserialize(cert_bytes)?;
    let trust = PublicKey::deserialize(trust_root_pub).map_err(|_| CryptoError::InvalidKey)?;
    let now = Timestamp::from_epoch_millis(validation_time_unix_millis);
    if !cert.validate(&trust, now)? {
        return Err(CryptoError::InvalidCiphertext);
    }
    Ok(SenderCertInfo {
        sender_did: cert.sender_uuid()?.to_string(),
        sender_device_id: u32::from(cert.sender_device_id()?),
        identity_key_pub: cert.key()?.serialize().to_vec(),
        expiration_unix_millis: cert.expiration()?.epoch_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_identity() -> Vec<u8> {
        // A real libsignal IdentityKey is a Curve25519 public key.
        let mut rng = OsRng.unwrap_err();
        let kp = libsignal_protocol::KeyPair::generate(&mut rng);
        kp.public_key.serialize().to_vec()
    }

    #[test]
    fn chain_roundtrips_through_bytes() {
        let chain = SenderCertChain::generate().expect("generate");
        let bytes = chain.to_bytes();
        let again = SenderCertChain::from_bytes(&bytes).expect("from_bytes");
        assert_eq!(again.trust_root_public_bytes(), chain.trust_root_public_bytes());
        assert_eq!(again.to_bytes(), bytes);
    }

    #[test]
    fn issue_validate_roundtrip() {
        let chain = SenderCertChain::generate().expect("generate");
        let ident = fake_identity();
        let now = 1_700_000_000_000u64;
        let exp = now + 2 * 86_400_000; // 2 days
        let cert = chain
            .issue_sender_cert("did:plc:alice", 1, &ident, exp)
            .expect("issue");
        let info = validate_sender_cert(&cert, &chain.trust_root_public_bytes(), now)
            .expect("validate");
        assert_eq!(info.sender_did, "did:plc:alice");
        assert_eq!(info.sender_device_id, 1);
        assert_eq!(info.identity_key_pub, ident);
        assert_eq!(info.expiration_unix_millis, exp);
    }

    #[test]
    fn validate_rejects_expired() {
        let chain = SenderCertChain::generate().expect("generate");
        let ident = fake_identity();
        let cert = chain
            .issue_sender_cert("did:plc:alice", 1, &ident, 1_000)
            .expect("issue");
        // validation_time is well past expiration
        assert!(validate_sender_cert(&cert, &chain.trust_root_public_bytes(), 9_999_999).is_err());
    }

    #[test]
    fn validate_rejects_wrong_trust_root() {
        let chain_a = SenderCertChain::generate().expect("generate");
        let chain_b = SenderCertChain::generate().expect("generate");
        let ident = fake_identity();
        let cert = chain_a
            .issue_sender_cert("did:plc:alice", 1, &ident, 1_700_000_000_000)
            .expect("issue");
        assert!(validate_sender_cert(
            &cert,
            &chain_b.trust_root_public_bytes(),
            1_600_000_000_000,
        )
        .is_err());
    }
}
