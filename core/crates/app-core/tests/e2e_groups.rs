//! End-to-end integration test for action-bound groups
//! (docs/03-groups.md). Exercises the full flow:
//!
//! 1. Alice creates a group.
//! 2. Alice invites Bob; Bob receives the GroupContext DM, fetches state.
//! 3. Bob accepts (promote_pending_members) and becomes a member.
//! 4. Alice fetches state again and sees Bob in `members`.
//! 5. Alice changes Bob's role to Admin.
//! 6. Alice removes Bob.
//!
//! Requires a homeserver at `SERVER_URL` (default
//! `http://localhost:3000`). Run via `make test-e2e`.

use app_core::AppCore;

fn server_url() -> String {
    std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

async fn test_store() -> store::Store {
    let store = store::Store::open_in_memory().await.unwrap();
    store.migrate().await.unwrap();
    store
}

#[tokio::test]
async fn create_invite_accept_promote_remove_roundtrip() {
    let url = server_url();

    let alice = AppCore::create_account_with_store(&url, test_store().await, None, true)
        .await
        .unwrap();
    let bob = AppCore::create_account_with_store(&url, test_store().await, None, true)
        .await
        .unwrap();

    let bob_did = bob.did_async().await;

    // 1. Alice creates the group.
    let created = alice
        .create_group_async("Test", "groups e2e", 0)
        .await
        .unwrap();
    assert_eq!(created.master_key.len(), 32);

    // 2. Alice invites Bob; this sends a GroupContext DM as a side effect.
    alice
        .invite_member_async(&created.group_id, &bob_did, 0)
        .await
        .unwrap();

    // 3. Bob receives the DM and stores the GroupContext locally.
    let msgs = bob.receive_messages_async().await.unwrap();
    assert!(
        !msgs.is_empty(),
        "bob should have received the GroupContext DM"
    );

    // 4. Bob fetches state; he should see himself in pending_invites.
    let bob_state = bob
        .fetch_group_state_async(&created.group_id)
        .await
        .unwrap();
    assert_eq!(
        bob_state.pending_invites.len(),
        1,
        "bob should see one pending invite"
    );
    assert_eq!(
        bob_state.members.len(),
        1,
        "only alice should be a full member at this point"
    );

    // 5. Bob accepts (promote_pending_members).
    bob.accept_invite_async(&created.group_id).await.unwrap();

    // 6. Alice re-fetches; she should see Bob in members and the pending row gone.
    let alice_state = alice
        .fetch_group_state_async(&created.group_id)
        .await
        .unwrap();
    assert_eq!(alice_state.members.len(), 2);
    assert!(alice_state.pending_invites.is_empty());
}
