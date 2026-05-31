//! Per-homeserver zkgroup signing material.
//!
//! Each homeserver holds exactly one [`ServerSecretParams`] — the signing
//! material it uses to issue anonymous auth credentials and group send
//! endorsements. The matching [`ServerPublicParams`] is published so clients
//! can verify issuances and produce presentations. Persistence (DB storage,
//! hot-reload on startup) is the server's job; this module only provides the
//! crypto-level primitives.
//!
//! What lives in here:
//! - the libsignal `zkgroup::ServerSecretParams` — covers the endorsement
//!   server root key pair we need for `GroupSendEndorsement` (group sends),
//!   plus other libsignal-internal credential keys we carry along for
//!   compatibility with the group send path;
//! - a dedicated `CredentialKeyPair` used by `AuthCredentialDid` (see
//!   `crypto::groups::credentials`). We need a separate one because
//!   `zkgroup::ServerSecretParams::generic_credential_key_pair` is
//!   `pub(crate)` to zkgroup with no public accessor.
//!
//! Wire format is a bincode-serialized pair of byte vectors:
//! `(zkgroup-bytes, auth-cred-bytes)`. Stable for a given pinned libsignal
//! commit. Changing it requires a migration; see
//! `infra/migrations/009_zkgroup_server_params.sql`.

use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use zkcredential::credentials::{CredentialKeyPair, CredentialPublicKey};

use crate::error::CryptoError;

/// Server-side signing material. Holding this is equivalent to holding the
/// homeserver's authority to issue auth credentials and group send
/// endorsements.
#[derive(Clone)]
pub struct ServerSecretParams {
    zkgroup: zkgroup::ServerSecretParams,
    auth_credential_key: CredentialKeyPair,
}

/// Public counterpart published to clients so they can verify credential
/// issuances and produce presentations.
#[derive(Clone)]
pub struct ServerPublicParams {
    zkgroup: zkgroup::ServerPublicParams,
    auth_credential_key: CredentialPublicKey,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    zkgroup: Vec<u8>,
    auth_credential_key: Vec<u8>,
}

fn fresh_randomness() -> [u8; zkgroup::RANDOMNESS_LEN] {
    let mut r = [0u8; zkgroup::RANDOMNESS_LEN];
    rand::rngs::OsRng
        .try_fill_bytes(&mut r)
        .expect("OS RNG failed");
    r
}

impl ServerSecretParams {
    /// Generate fresh material. Uses two independent draws from the OS RNG so
    /// the auth credential key isn't correlated with the zkgroup material.
    pub fn generate() -> Self {
        let zkgroup = zkgroup::ServerSecretParams::generate(fresh_randomness());
        let auth_credential_key = CredentialKeyPair::generate(fresh_randomness());
        Self {
            zkgroup,
            auth_credential_key,
        }
    }

    pub fn public_params(&self) -> ServerPublicParams {
        ServerPublicParams {
            zkgroup: self.zkgroup.get_public_params(),
            auth_credential_key: self.auth_credential_key.public_key().clone(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let wire = Wire {
            zkgroup: zkgroup::serialize(&self.zkgroup),
            auth_credential_key: bincode::serialize(&self.auth_credential_key)
                .expect("serialize CredentialKeyPair"),
        };
        bincode::serialize(&wire).expect("serialize ServerSecretParams wire")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let wire: Wire =
            bincode::deserialize(bytes).map_err(|_| CryptoError::ZkgroupDeserialize)?;
        let zkgroup = zkgroup::deserialize::<zkgroup::ServerSecretParams>(&wire.zkgroup)
            .map_err(|_| CryptoError::ZkgroupDeserialize)?;
        let auth_credential_key: CredentialKeyPair =
            bincode::deserialize(&wire.auth_credential_key)
                .map_err(|_| CryptoError::ZkgroupDeserialize)?;
        Ok(Self {
            zkgroup,
            auth_credential_key,
        })
    }

    /// Accessor for the `AuthCredentialDid` issuance key. Crate-private so
    /// callers outside `crypto::groups` stay scheme-agnostic.
    pub(crate) fn auth_credential_key(&self) -> &CredentialKeyPair {
        &self.auth_credential_key
    }

    #[allow(dead_code)] // used by group send endorsements, landing in a later step
    pub(crate) fn zkgroup(&self) -> &zkgroup::ServerSecretParams {
        &self.zkgroup
    }
}

impl ServerPublicParams {
    pub fn to_bytes(&self) -> Vec<u8> {
        let wire = Wire {
            zkgroup: zkgroup::serialize(&self.zkgroup),
            auth_credential_key: bincode::serialize(&self.auth_credential_key)
                .expect("serialize CredentialPublicKey"),
        };
        bincode::serialize(&wire).expect("serialize ServerPublicParams wire")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let wire: Wire =
            bincode::deserialize(bytes).map_err(|_| CryptoError::ZkgroupDeserialize)?;
        let zkgroup = zkgroup::deserialize::<zkgroup::ServerPublicParams>(&wire.zkgroup)
            .map_err(|_| CryptoError::ZkgroupDeserialize)?;
        let auth_credential_key: CredentialPublicKey =
            bincode::deserialize(&wire.auth_credential_key)
                .map_err(|_| CryptoError::ZkgroupDeserialize)?;
        Ok(Self {
            zkgroup,
            auth_credential_key,
        })
    }

    pub(crate) fn auth_credential_key(&self) -> &CredentialPublicKey {
        &self.auth_credential_key
    }

    #[allow(dead_code)] // used by group send endorsements, landing in a later step
    pub(crate) fn zkgroup(&self) -> &zkgroup::ServerPublicParams {
        &self.zkgroup
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
