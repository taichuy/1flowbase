use control_plane::ports::SessionStore;
use domain::SessionRecord;
use storage_ephemeral::MemorySessionStore;
use time::OffsetDateTime;
use uuid::Uuid;

fn fixture_session_with_expiry(expires_at_unix: i64) -> SessionRecord {
    SessionRecord {
        session_id: "session-1".into(),
        user_id: Uuid::now_v7(),
        tenant_id: Uuid::now_v7(),
        current_workspace_id: Uuid::now_v7(),
        active_role_code: "member".into(),
        session_version: 1,
        csrf_token: "csrf-1".into(),
        expires_at_unix,
    }
}

#[tokio::test]
async fn memory_session_store_drops_expired_session_on_get() {
    let store = MemorySessionStore::new("flowbase:test");
    let expired = fixture_session_with_expiry(OffsetDateTime::now_utc().unix_timestamp() - 1);

    store.put(expired).await.unwrap();

    assert!(store.get("session-1").await.unwrap().is_none());
}

#[tokio::test]
async fn memory_session_store_touch_extends_expiry() {
    let store = MemorySessionStore::new("flowbase:test");
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let session = fixture_session_with_expiry(now + 1);

    store.put(session.clone()).await.unwrap();
    store.touch(&session.session_id, now + 120).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    assert!(store.get(&session.session_id).await.unwrap().is_some());
}
