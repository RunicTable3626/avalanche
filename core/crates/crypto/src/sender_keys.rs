//! Sender Keys ratchet wrappers. Used by zkgroup action-bound groups and
//! (Stage 9) cross-server casual groups to encrypt message *content* —
//! distinct from the per-pairwise Double Ratchet used for DMs.
//!
//! Conceptually:
//! - Each member owns one `SenderKey` per `(group, member)` slot,
//!   identified by a `distribution_id` (a UUID derived deterministically
//!   from the group master key — see `app_core::groups`). All members of
//!   a group compute the same `distribution_id`; the ratchet state for
//!   each *sender* is stored separately under their own
//!   `ProtocolAddress`.
//! - At join/invite time, each member sends out a
//!   `SenderKeyDistributionMessage` (SKDM) over their existing pairwise
//!   Signal session to every other member. Recipients call
//!   [`process_skdm`] to install the sender's key into their local
//!   `SenderKeyStore`.
//! - Senders call [`group_encrypt`] to encrypt a message; recipients call
//!   [`group_decrypt`] using the cached sender key for that member.
//!
//! These four functions are thin wrappers around libsignal's
//! `group_cipher` API so callers don't need to depend on `libsignal-
//! protocol` directly and so the byte-vs-typed boundary is consistent
//! with the rest of the `crypto` crate (in/out are bytes; intermediate
//! libsignal types stay inside this module).

use libsignal_protocol::{self as signal, SenderKeyStore};
use rand::rngs::OsRng;
use rand::TryRngCore as _;
use uuid::Uuid;

use crate::error::CryptoError;

/// Generate (or rotate) the local sender key for `(sender, distribution_id)`
/// and return the wire bytes of the distribution message the caller ships
/// to every other group member over their pairwise Signal session.
///
/// Idempotent in the sense that if a record already exists for the slot,
/// libsignal reuses its chain — same return value across calls within a
/// chain. Callers who want a *fresh* chain (e.g. after a member is
/// removed and forward secrecy must reset) should delete the existing
/// record first.
pub async fn create_skdm(
    store: &mut dyn SenderKeyStore,
    sender: &signal::ProtocolAddress,
    distribution_id: Uuid,
) -> Result<Vec<u8>, CryptoError> {
    let mut csprng = OsRng.unwrap_err();
    let skdm = signal::create_sender_key_distribution_message(
        sender,
        distribution_id,
        store,
        &mut csprng,
    )
    .await?;
    Ok(skdm.serialized().to_vec())
}

/// Receive someone else's distribution message and install their sender
/// key under their `(sender, distribution_id)` slot. Subsequent
/// [`group_decrypt`] calls for messages from that sender will succeed.
pub async fn process_skdm(
    store: &mut dyn SenderKeyStore,
    sender: &signal::ProtocolAddress,
    skdm_bytes: &[u8],
) -> Result<(), CryptoError> {
    let skdm = signal::SenderKeyDistributionMessage::try_from(skdm_bytes)?;
    signal::process_sender_key_distribution_message(sender, &skdm, store).await?;
    Ok(())
}

/// Encrypt `plaintext` under the local sender key for
/// `(sender, distribution_id)`. Returns the wire bytes of a
/// `SenderKeyMessage` that any recipient who has already processed our
/// SKDM can decrypt.
pub async fn group_encrypt(
    store: &mut dyn SenderKeyStore,
    sender: &signal::ProtocolAddress,
    distribution_id: Uuid,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut csprng = OsRng.unwrap_err();
    let skm = signal::group_encrypt(store, sender, distribution_id, plaintext, &mut csprng).await?;
    Ok(skm.serialized().to_vec())
}

/// Decrypt `ciphertext` (wire bytes of a `SenderKeyMessage`) assuming the
/// sender's SKDM has already been installed via [`process_skdm`]. The
/// `distribution_id` carried inside the ciphertext is matched against
/// stored state automatically; callers only need to identify the
/// sending member by their `ProtocolAddress`.
pub async fn group_decrypt(
    store: &mut dyn SenderKeyStore,
    sender: &signal::ProtocolAddress,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let plaintext = signal::group_decrypt(ciphertext, store, sender).await?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;

    /// Minimal in-memory `SenderKeyStore` for tests.
    #[derive(Default)]
    struct InMemoryStore {
        records: HashMap<(String, Uuid), Vec<u8>>,
    }

    fn key(addr: &signal::ProtocolAddress) -> String {
        format!("{}.{}", addr.name(), u32::from(addr.device_id()))
    }

    #[async_trait(?Send)]
    impl SenderKeyStore for InMemoryStore {
        async fn store_sender_key(
            &mut self,
            sender: &signal::ProtocolAddress,
            distribution_id: Uuid,
            record: &signal::SenderKeyRecord,
        ) -> Result<(), signal::SignalProtocolError> {
            self.records
                .insert((key(sender), distribution_id), record.serialize()?);
            Ok(())
        }

        async fn load_sender_key(
            &mut self,
            sender: &signal::ProtocolAddress,
            distribution_id: Uuid,
        ) -> Result<Option<signal::SenderKeyRecord>, signal::SignalProtocolError> {
            match self.records.get(&(key(sender), distribution_id)) {
                Some(bytes) => Ok(Some(signal::SenderKeyRecord::deserialize(bytes)?)),
                None => Ok(None),
            }
        }
    }

    fn addr(name: &str) -> signal::ProtocolAddress {
        signal::ProtocolAddress::new(name.into(), 1u32.try_into().unwrap())
    }

    #[tokio::test]
    async fn skdm_roundtrip_and_encrypt_decrypt() {
        let dist_id = Uuid::new_v4();
        let alice_addr = addr("alice");

        // Alice's local store has her sender key after SKDM creation.
        let mut alice_store = InMemoryStore::default();
        let skdm = create_skdm(&mut alice_store, &alice_addr, dist_id)
            .await
            .expect("create_skdm");

        // Bob receives the SKDM and installs Alice's sender key.
        let mut bob_store = InMemoryStore::default();
        process_skdm(&mut bob_store, &alice_addr, &skdm)
            .await
            .expect("process_skdm");

        // Alice encrypts; Bob decrypts.
        let ct = group_encrypt(&mut alice_store, &alice_addr, dist_id, b"hello group")
            .await
            .expect("group_encrypt");
        let pt = group_decrypt(&mut bob_store, &alice_addr, &ct)
            .await
            .expect("group_decrypt");
        assert_eq!(pt, b"hello group");
    }

    #[tokio::test]
    async fn group_decrypt_fails_without_skdm() {
        let dist_id = Uuid::new_v4();
        let alice_addr = addr("alice");

        let mut alice_store = InMemoryStore::default();
        let _ = create_skdm(&mut alice_store, &alice_addr, dist_id).await.unwrap();
        let ct = group_encrypt(&mut alice_store, &alice_addr, dist_id, b"secret")
            .await
            .unwrap();

        // Bob never processed the SKDM.
        let mut bob_store = InMemoryStore::default();
        assert!(group_decrypt(&mut bob_store, &alice_addr, &ct).await.is_err());
    }
}
