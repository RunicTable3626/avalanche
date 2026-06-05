//! Tests that complement `integration.rs`. These cover store edge cases
//! not exercised by the end-to-end session tests.

use store::Store;

#[tokio::test]
async fn load_identity_returns_none_before_creation() {
    let store = Store::open_in_memory().await.unwrap();
    assert!(store.load_identity().await.unwrap().is_none());
}

#[tokio::test]
async fn store_satisfies_crypto_store_trait() {
    use crypto::Store as CryptoStore;

    let store = Store::open_in_memory().await.unwrap();
    let identity = crypto::IdentityKeyPair::generate();
    store.save_identity(&identity, 100).await.unwrap();

    fn assert_is_crypto_store(_s: &impl CryptoStore) {}
    assert_is_crypto_store(&store);
}

#[tokio::test]
async fn message_queue_ordering() {
    use store::messages::QueuedMessage;
    use types::{MessageId, Timestamp};

    let store = Store::open_in_memory().await.unwrap();

    // Enqueue out of chronological order.
    let msg_later = QueuedMessage {
        id: MessageId::new(),
        recipient_name: "bob".to_string(),
        recipient_device_id: 1,
        ciphertext: vec![2],
        message_kind: 0,
        enqueued_at: Timestamp(2000),
    };
    let msg_earlier = QueuedMessage {
        id: MessageId::new(),
        recipient_name: "carol".to_string(),
        recipient_device_id: 1,
        ciphertext: vec![1],
        message_kind: 0,
        enqueued_at: Timestamp(1000),
    };

    store.enqueue(&msg_later).await.unwrap();
    store.enqueue(&msg_earlier).await.unwrap();

    let drained = store.drain().await.unwrap();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].recipient_name, "carol", "earlier message should come first");
    assert_eq!(drained[1].recipient_name, "bob");
}

#[tokio::test]
async fn own_profile_round_trip() {
    let store = Store::open_in_memory().await.unwrap();
    assert!(store.load_own_profile().await.unwrap().is_none());

    let profile = store::profiles::OwnProfile {
        profile_key: vec![7u8; 32],
        display_name: "Alice".into(),
    };
    store.save_own_profile(&profile).await.unwrap();

    let loaded = store.load_own_profile().await.unwrap().unwrap();
    assert_eq!(loaded.profile_key, vec![7u8; 32]);
    assert_eq!(loaded.display_name, "Alice");

    store.update_own_display_name("Alice Updated").await.unwrap();
    let loaded = store.load_own_profile().await.unwrap().unwrap();
    assert_eq!(loaded.display_name, "Alice Updated");
    assert_eq!(loaded.profile_key, vec![7u8; 32], "key unchanged on rename");
}

#[tokio::test]
async fn load_conversations_one_row_per_convo_newest_first() {
    use store::messages::HistoryMessage;
    use types::Timestamp;
    let store = Store::open_in_memory().await.unwrap();

    // Two messages in convA (newest at t=1000), one in convB (t=500).
    for (id, conv, sent_at, body) in [
        ("a1", "convA", 100i64, "older A"),
        ("a2", "convA", 1000i64, "newest A"),
        ("b1", "convB", 500i64, "only B"),
    ] {
        store.save_message(&HistoryMessage {
            id: id.into(),
            conversation_id: conv.into(),
            sender_did: "did:plc:bob".into(),
            body: body.into(),
            sent_at: Timestamp(sent_at),
            edited_at: None,
            read_at: None,
            delivery_status: 1,
        }).await.unwrap();
    }

    let convs = store.load_conversations().await.unwrap();
    assert_eq!(convs.len(), 2, "one row per distinct conversation_id");
    assert_eq!(convs[0].conversation_id, "convA");
    assert_eq!(convs[0].last_message.unwrap().body, "newest A");
    assert_eq!(convs[1].conversation_id, "convB");
    assert_eq!(convs[1].last_message.unwrap().body, "only B");
}

#[tokio::test]
async fn contact_profile_cache() {
    use types::Timestamp;
    let store = Store::open_in_memory().await.unwrap();

    let did = "did:plc:bob000000000000000001";
    assert!(store.load_contact_profile(did).await.unwrap().is_none());

    let p = store::profiles::ContactProfile {
        did: did.into(),
        display_name: "Bob".into(),
        profile_key: vec![9u8; 32],
        fetched_at: Timestamp(1000),
    };
    store.upsert_contact_profile(&p).await.unwrap();

    let loaded = store.load_contact_profile(did).await.unwrap().unwrap();
    assert_eq!(loaded.display_name, "Bob");
    assert_eq!(loaded.profile_key, vec![9u8; 32]);

    let key = store.load_contact_profile_key(did).await.unwrap().unwrap();
    assert_eq!(key, vec![9u8; 32]);

    let p2 = store::profiles::ContactProfile {
        did: did.into(),
        display_name: "Bob (renamed)".into(),
        profile_key: vec![9u8; 32],
        fetched_at: Timestamp(2000),
    };
    store.upsert_contact_profile(&p2).await.unwrap();
    let loaded = store.load_contact_profile(did).await.unwrap().unwrap();
    assert_eq!(loaded.display_name, "Bob (renamed)");
    assert_eq!(loaded.fetched_at, Timestamp(2000));
}
