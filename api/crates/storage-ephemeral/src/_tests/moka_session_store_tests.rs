use control_plane::ports::{EphemeralValueRevealMode, SessionStore};
use domain::SessionRecord;
use storage_ephemeral::MokaSessionStore;
use time::OffsetDateTime;
use uuid::Uuid;

fn fixture_session_with_expiry(expires_at_unix: i64) -> SessionRecord {
    SessionRecord {
        session_id: "session-1".to_string(),
        user_id: Uuid::now_v7(),
        tenant_id: Uuid::now_v7(),
        current_workspace_id: Uuid::now_v7(),
        active_role_code: "member".into(),
        session_version: 1,
        csrf_token: "csrf-1".to_string(),
        expires_at_unix,
    }
}

#[tokio::test]
async fn moka_session_store_put_get_touch_and_delete() {
    let store = MokaSessionStore::new("flowbase:session", 128);
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut session = fixture_session_with_expiry(now + 60);

    store.put(session.clone()).await.unwrap();
    assert_eq!(store.get("session-1").await.unwrap(), Some(session.clone()));

    session.expires_at_unix += 60;
    store
        .touch("session-1", session.expires_at_unix)
        .await
        .unwrap();
    assert_eq!(
        store
            .get("session-1")
            .await
            .unwrap()
            .unwrap()
            .expires_at_unix,
        session.expires_at_unix
    );

    store.delete("session-1").await.unwrap();
    assert_eq!(store.get("session-1").await.unwrap(), None);
}

#[tokio::test]
async fn moka_session_store_drops_expired_session_on_get() {
    let store = MokaSessionStore::new("flowbase:session", 128);
    let expired = fixture_session_with_expiry(OffsetDateTime::now_utc().unix_timestamp() - 1);

    store.put(expired).await.unwrap();

    assert_eq!(store.get("session-1").await.unwrap(), None);
}

#[tokio::test]
async fn moka_session_store_touch_with_expired_deadline_deletes_session() {
    let store = MokaSessionStore::new("flowbase:session", 128);
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let session = fixture_session_with_expiry(now + 60);

    store.put(session).await.unwrap();
    store.touch("session-1", now - 1).await.unwrap();

    assert_eq!(store.get("session-1").await.unwrap(), None);
}

#[tokio::test]
async fn moka_session_store_exposes_ephemeral_inspection_snapshots() {
    let store = MokaSessionStore::new("flowbase:session", 128);
    let session = fixture_session_with_expiry(OffsetDateTime::now_utc().unix_timestamp() + 60);

    store.put(session.clone()).await.unwrap();

    let capabilities = store.ephemeral_inspection_capabilities();
    assert!(capabilities.list_entries);
    assert!(capabilities.reveal_value);
    let entries = store.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contract_code, "session-store");
    assert_eq!(entries[0].key, session.session_id);
    assert_eq!(entries[0].owner, Some(session.user_id.to_string()));
    assert!(entries[0].sensitive);

    let revealed = store
        .reveal_ephemeral_entry(&session.session_id, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata.key, session.session_id);
    assert_eq!(revealed.value.unwrap()["session_id"], session.session_id);
}
