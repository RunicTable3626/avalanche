//! Recovery blob creation, encryption, and decryption.
//!
//! The blob is a server-side cache that lets a recovering device skip
//! re-registration friction. It holds the device identity keypair, the
//! server list, and profile data. **It does NOT contain the rotation key** —
//! the rotation key is deterministically re-derived from the passkey on every
//! recovery via [`derive_recovery_keys_from_prf`], so the blob never needs to
//! carry DID-controlling authority.
//!
//! Encryption: AES-256-GCM with a random 12-byte nonce, using the blob key
//! derived from the passkey PRF output via HKDF (label `"actnet-blob-v1"`).
//! Wire format: `version (1 byte) || nonce (12 bytes) || ciphertext || tag (16 bytes)`
//!
//! Always bump [`RECOVERY_BLOB_VERSION`] when changing the plaintext schema
//! or the wire format.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const NONCE_LEN: usize = 12;

/// Current recovery-blob wire-format version. Bump whenever the byte layout
/// or plaintext schema changes in a non-backward-compatible way. Decryption
/// rejects unknown versions so old clients fail loudly rather than parsing
/// garbage.
///
/// v2 dropped the `rotation_key` field — the rotation key is now derived
/// from the passkey via HKDF at recovery time, not stored in the blob.
pub const RECOVERY_BLOB_VERSION: u8 = 2;

/// Plaintext contents of a recovery blob (v2 schema).
#[derive(Serialize, Deserialize)]
pub struct RecoveryBlobPlaintext {
    /// libsignal identity keypair (serialized bytes, base64). Generated
    /// randomly per device at signup; restored from the blob to preserve
    /// safety numbers across device migrations.
    pub identity_keypair: String,
    /// List of homeserver URLs the user is registered on.
    pub servers: Vec<String>,
    /// 32-byte profile key (base64). Used to encrypt the user's display
    /// name into the server-stored profile blob. Restoring it on recovery
    /// keeps existing contacts pointed at the same profile blob, so their
    /// cached display name stays valid.
    #[serde(default)]
    pub profile_key: String,
    /// User's display name in plaintext (mirrors what the server-side
    /// encrypted profile blob decrypts to). Stored locally as
    /// `own_profile.display_name`; carried in the recovery blob so a
    /// fresh device can restore it without prompting.
    #[serde(default)]
    pub display_name: String,
}

/// Encrypt a recovery blob with a 32-byte symmetric key.
pub fn encrypt_recovery_blob(
    plaintext: &RecoveryBlobPlaintext,
    symmetric_key: &[u8; 32],
) -> Result<Vec<u8>, AppError> {
    let json = serde_json::to_vec(plaintext)
        .map_err(|e| AppError::Protocol(format!("failed to serialize recovery blob: {e}")))?;

    let cipher = Aes256Gcm::new(symmetric_key.into());
    let nonce_bytes: [u8; NONCE_LEN] = rand::Rng::random(&mut rand::rng());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, json.as_slice())
        .map_err(|e| AppError::Protocol(format!("recovery blob encryption failed: {e}")))?;

    // version || nonce || ciphertext (includes GCM tag)
    let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    blob.push(RECOVERY_BLOB_VERSION);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

/// Decrypt a recovery blob with a 32-byte symmetric key.
pub fn decrypt_recovery_blob(
    blob: &[u8],
    symmetric_key: &[u8; 32],
) -> Result<RecoveryBlobPlaintext, AppError> {
    if blob.len() < 1 + NONCE_LEN + 16 {
        return Err(AppError::Protocol("recovery blob too short".into()));
    }

    let version = blob[0];
    if version != RECOVERY_BLOB_VERSION {
        return Err(AppError::Protocol(format!(
            "unsupported recovery blob version: {version} (expected {RECOVERY_BLOB_VERSION})"
        )));
    }

    let (nonce_bytes, ciphertext) = blob[1..].split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(symmetric_key.into());
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::Protocol("recovery blob decryption failed (wrong key?)".into()))?;

    serde_json::from_slice(&plaintext)
        .map_err(|e| AppError::Protocol(format!("recovery blob JSON parse failed: {e}")))
}

/// HKDF labels used to split the passkey PRF output into purpose-bound keys.
/// Versioned so future schema changes don't silently reuse the same bytes.
const HKDF_LABEL_ROTATION: &[u8] = b"actnet-rotation-v1";
const HKDF_LABEL_BLOB: &[u8] = b"actnet-blob-v1";

/// Derive the rotation key seed and the blob-encryption key from the raw
/// 32-byte PRF output (or any other high-entropy seed of equivalent length,
/// such as the bytes derived from a written-down recovery phrase).
///
/// Returns `(rotation_seed, blob_key)`. Both are 32 bytes.
/// - `rotation_seed` must be passed to [`derive_rotation_key_from_seed`]
///   to obtain the actual P-256 keypair.
/// - `blob_key` is the AES-256-GCM key that encrypts/decrypts the recovery blob.
pub fn derive_recovery_keys_from_prf(prf_output: &[u8]) -> ([u8; 32], [u8; 32]) {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hk = Hkdf::<Sha256>::new(None, prf_output);
    let mut rotation_seed = [0u8; 32];
    let mut blob_key = [0u8; 32];
    hk.expand(HKDF_LABEL_ROTATION, &mut rotation_seed)
        .expect("HKDF expand never fails for 32-byte output");
    hk.expand(HKDF_LABEL_BLOB, &mut blob_key)
        .expect("HKDF expand never fails for 32-byte output");
    (rotation_seed, blob_key)
}

/// Deterministically derive a P-256 rotation keypair from a 32-byte seed.
///
/// Returns `(private_key_sec1_bytes, public_key_sec1_compressed_bytes)`.
/// The seed must be from a high-entropy source (the `rotation_seed` output
/// of [`derive_recovery_keys_from_prf`], not arbitrary user input).
///
/// Reduces the seed mod the P-256 group order to obtain a scalar in
/// `[1, n-1]`. The probability of landing on zero is negligible for any
/// real PRF output; we retry once via `+1` as a paranoid safety net.
pub fn derive_rotation_key_from_seed(seed: &[u8; 32]) -> (Vec<u8>, Vec<u8>) {
    use p256::ecdsa::SigningKey;
    use p256::elliptic_curve::generic_array::GenericArray;

    // SigningKey::from_bytes interprets the input as a scalar and rejects
    // zero; we accept its result on any non-zero seed (overwhelmingly likely).
    let arr = GenericArray::clone_from_slice(seed);
    let signing_key = SigningKey::from_bytes(&arr).unwrap_or_else(|_| {
        // Vanishingly improbable: seed reduces to zero. Tweak and retry.
        let mut tweak = *seed;
        tweak[31] ^= 0x01;
        let arr2 = GenericArray::clone_from_slice(&tweak);
        SigningKey::from_bytes(&arr2).expect("tweaked seed is non-zero")
    });
    let private_bytes = signing_key.to_bytes().to_vec();
    let public_bytes = signing_key
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .to_vec();
    (private_bytes, public_bytes)
}

/// Generate a random P-256 rotation key. Used only when the user skips
/// passkey creation — without a passkey there is no PRF output to derive
/// from, so the rotation key has no recoverable user-held source. The
/// identity is effectively unrecoverable on device loss; surfaces in the
/// "skip recovery" path documented in `33-identity-auth-recovery.md`.
pub fn generate_rotation_key() -> (Vec<u8>, Vec<u8>) {
    use p256::ecdsa::SigningKey;
    let signing_key = SigningKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
    let private_bytes = signing_key.to_bytes().to_vec();
    let public_bytes = signing_key
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .to_vec();
    (private_bytes, public_bytes)
}

/// Sign a payload with a P-256 rotation key. Returns DER-encoded ECDSA signature.
pub fn sign_with_rotation_key(
    private_key_bytes: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>, AppError> {
    use p256::ecdsa::{signature::Signer, Signature, SigningKey};

    let signing_key = SigningKey::from_bytes(private_key_bytes.into())
        .map_err(|e| AppError::Protocol(format!("invalid rotation key: {e}")))?;
    let sig: Signature = signing_key.sign(payload);
    Ok(sig.to_der().as_bytes().to_vec())
}

/// Build a recovery blob plaintext from the current account state.
///
/// `profile_key` is the 32-byte symmetric key that encrypts the user's
/// profile blob on each homeserver. Pass `&[]` to omit (e.g. for bot
/// accounts that have no profile).
pub fn build_recovery_plaintext(
    identity_keypair_bytes: &[u8],
    servers: &[String],
    profile_key: &[u8],
    display_name: &str,
) -> RecoveryBlobPlaintext {
    RecoveryBlobPlaintext {
        identity_keypair: BASE64_STANDARD.encode(identity_keypair_bytes),
        servers: servers.to_vec(),
        profile_key: if profile_key.is_empty() {
            String::new()
        } else {
            BASE64_STANDARD.encode(profile_key)
        },
        display_name: display_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_recovery_blob() {
        let key = [42u8; 32];
        let plaintext = RecoveryBlobPlaintext {
            identity_keypair: BASE64_STANDARD.encode(b"fake-identity-keypair"),
            servers: vec!["https://server1.example".into(), "https://server2.example".into()],
            profile_key: BASE64_STANDARD.encode([7u8; 32]),
            display_name: "Sam".into(),
        };

        let blob = encrypt_recovery_blob(&plaintext, &key).unwrap();
        let decrypted = decrypt_recovery_blob(&blob, &key).unwrap();

        assert_eq!(decrypted.identity_keypair, plaintext.identity_keypair);
        assert_eq!(decrypted.servers, plaintext.servers);
        assert_eq!(decrypted.profile_key, plaintext.profile_key);
        assert_eq!(decrypted.display_name, plaintext.display_name);
    }

    #[test]
    fn unknown_version_byte_rejected() {
        let key = [42u8; 32];
        let plaintext = RecoveryBlobPlaintext {
            identity_keypair: "dGVzdA==".into(),
            servers: vec![],
            profile_key: String::new(),
            display_name: String::new(),
        };
        let mut blob = encrypt_recovery_blob(&plaintext, &key).unwrap();
        blob[0] = 0xFF;
        let result = decrypt_recovery_blob(&blob, &key);
        let err_msg = match result {
            Err(e) => format!("{e:?}"),
            Ok(_) => panic!("expected version rejection"),
        };
        assert!(err_msg.contains("unsupported recovery blob version"), "got: {err_msg}");
    }

    #[test]
    fn encoded_blob_starts_with_version_byte() {
        let key = [42u8; 32];
        let plaintext = RecoveryBlobPlaintext {
            identity_keypair: "dGVzdA==".into(),
            servers: vec![],
            profile_key: String::new(),
            display_name: String::new(),
        };
        let blob = encrypt_recovery_blob(&plaintext, &key).unwrap();
        assert_eq!(blob[0], RECOVERY_BLOB_VERSION);
    }

    #[test]
    fn wrong_key_fails() {
        let key = [42u8; 32];
        let wrong_key = [99u8; 32];
        let plaintext = RecoveryBlobPlaintext {
            identity_keypair: "dGVzdA==".into(),
            servers: vec![],
            profile_key: String::new(),
            display_name: String::new(),
        };

        let blob = encrypt_recovery_blob(&plaintext, &key).unwrap();
        assert!(decrypt_recovery_blob(&blob, &wrong_key).is_err());
    }

    #[test]
    fn random_rotation_key_round_trip() {
        let (private_key, _public_key) = generate_rotation_key();
        let payload = b"replace:did:plc:test:1:2:nonce123";
        let sig = sign_with_rotation_key(&private_key, payload).unwrap();
        assert!(!sig.is_empty());

        use p256::ecdsa::{signature::Verifier, Signature, SigningKey, VerifyingKey};
        let signing_key = SigningKey::from_bytes((&private_key[..]).into()).unwrap();
        let verifying_key = VerifyingKey::from(&signing_key);
        let signature = Signature::from_der(&sig).unwrap();
        verifying_key.verify(payload, &signature).unwrap();
    }

    #[test]
    fn prf_derivation_is_deterministic() {
        let prf = [7u8; 32];
        let (rot_seed_1, blob_key_1) = derive_recovery_keys_from_prf(&prf);
        let (rot_seed_2, blob_key_2) = derive_recovery_keys_from_prf(&prf);
        assert_eq!(rot_seed_1, rot_seed_2);
        assert_eq!(blob_key_1, blob_key_2);
        // Labels must produce distinct outputs.
        assert_ne!(rot_seed_1, blob_key_1);
    }

    #[test]
    fn derived_rotation_key_signs_and_verifies() {
        let prf = [42u8; 32];
        let (seed, _blob_key) = derive_recovery_keys_from_prf(&prf);
        let (priv1, pub1) = derive_rotation_key_from_seed(&seed);
        let (priv2, pub2) = derive_rotation_key_from_seed(&seed);
        assert_eq!(priv1, priv2, "derivation is deterministic");
        assert_eq!(pub1, pub2);

        let payload = b"test-payload";
        let sig = sign_with_rotation_key(&priv1, payload).unwrap();

        use p256::ecdsa::{signature::Verifier, Signature, SigningKey, VerifyingKey};
        let signing_key = SigningKey::from_bytes((&priv1[..]).into()).unwrap();
        let verifying_key = VerifyingKey::from(&signing_key);
        let signature = Signature::from_der(&sig).unwrap();
        verifying_key.verify(payload, &signature).unwrap();
    }

    #[test]
    fn different_prf_gives_different_rotation_key() {
        let (seed_a, _) = derive_recovery_keys_from_prf(&[1u8; 32]);
        let (seed_b, _) = derive_recovery_keys_from_prf(&[2u8; 32]);
        let (priv_a, _) = derive_rotation_key_from_seed(&seed_a);
        let (priv_b, _) = derive_rotation_key_from_seed(&seed_b);
        assert_ne!(priv_a, priv_b);
    }
}
