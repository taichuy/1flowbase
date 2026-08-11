use async_trait::async_trait;
use control_plane::ports::SessionStore;
use domain::SessionRecord;

use crate::{
    session_store::{is_session_expired, session_ttl},
    EphemeralKvStore, MemoryKvStore,
};

#[derive(Clone)]
pub struct MemorySessionStore {
    kv: MemoryKvStore,
}

impl MemorySessionStore {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            kv: MemoryKvStore::new(namespace),
        }
    }

    async fn read_session(&self, session_id: &str) -> anyhow::Result<Option<SessionRecord>> {
        self.kv
            .get_json(session_id)
            .await?
            .map(serde_json::from_value::<SessionRecord>)
            .transpose()
            .map_err(Into::into)
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn put(&self, session: SessionRecord) -> anyhow::Result<()> {
        self.kv
            .set_json(
                &session.session_id,
                serde_json::to_value(&session)?,
                Some(session_ttl(session.expires_at_unix)),
            )
            .await
    }

    async fn get(&self, session_id: &str) -> anyhow::Result<Option<SessionRecord>> {
        let Some(session) = self.read_session(session_id).await? else {
            return Ok(None);
        };

        if is_session_expired(&session) {
            self.delete(session_id).await?;
            return Ok(None);
        }

        Ok(Some(session))
    }

    async fn delete(&self, session_id: &str) -> anyhow::Result<()> {
        self.kv.delete(session_id).await
    }

    async fn touch(&self, session_id: &str, expires_at_unix: i64) -> anyhow::Result<()> {
        let ttl = session_ttl(expires_at_unix);
        if ttl.is_negative() || ttl.is_zero() {
            self.delete(session_id).await?;
            return Ok(());
        }

        let Some(mut session) = self.read_session(session_id).await? else {
            return Ok(());
        };
        if is_session_expired(&session) {
            self.delete(session_id).await?;
            return Ok(());
        }

        session.expires_at_unix = expires_at_unix;
        self.kv
            .set_json(session_id, serde_json::to_value(&session)?, Some(ttl))
            .await?;
        Ok(())
    }

    async fn reset_for_system_recovery(&self) -> anyhow::Result<()> {
        self.kv.clear_namespace().await;
        Ok(())
    }
}
