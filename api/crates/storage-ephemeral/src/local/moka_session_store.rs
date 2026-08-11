use async_trait::async_trait;
use control_plane::ports::{
    ephemeral_metadata_size_bytes, EphemeralEntrySnapshot, EphemeralEntryValueSnapshot,
    EphemeralInspectionCapabilities, EphemeralValueRevealMode, SessionStore,
};
use domain::SessionRecord;
use time::OffsetDateTime;

use crate::{
    session_store::{is_session_expired, session_ttl},
    EphemeralKvStore, MokaCacheStore,
};

#[derive(Clone)]
pub struct MokaSessionStore {
    kv: MokaCacheStore,
}

impl MokaSessionStore {
    pub fn new(namespace: impl Into<String>, max_capacity: u64) -> Self {
        Self {
            kv: MokaCacheStore::new(namespace, max_capacity),
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

    fn session_snapshot(
        session: &SessionRecord,
        value_size_bytes: u64,
        created_at_unix: Option<i64>,
    ) -> EphemeralEntrySnapshot {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let metadata = serde_json::json!({
            "tenant_id": session.tenant_id,
            "current_workspace_id": session.current_workspace_id,
            "session_version": session.session_version,
        });
        EphemeralEntrySnapshot {
            contract_code: "session-store".to_string(),
            group_code: Some(session.current_workspace_id.to_string()),
            entry_ref: session.session_id.clone(),
            key: session.session_id.clone(),
            inspection_path: vec![
                session.current_workspace_id.to_string(),
                session.user_id.to_string(),
                session.session_id.clone(),
            ],
            entry_kind: "session".to_string(),
            status: "active".to_string(),
            owner: Some(session.user_id.to_string()),
            value_size_bytes,
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: Some((session.expires_at_unix - now).max(0)),
            created_at_unix,
            expires_at_unix: Some(session.expires_at_unix),
            sensitive: true,
            metadata,
        }
    }
}

#[async_trait]
impl SessionStore for MokaSessionStore {
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
        if ttl <= time::Duration::ZERO {
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
            .await
    }

    async fn reset_for_system_recovery(&self) -> anyhow::Result<()> {
        <MokaCacheStore as control_plane::ports::CacheStore>::reset_for_system_recovery(&self.kv)
            .await
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::supported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        let mut entries = self
            .kv
            .list_json_entries_for_inspection()
            .await
            .into_iter()
            .filter_map(|entry| {
                let session = serde_json::from_value::<SessionRecord>(entry.value).ok()?;
                if is_session_expired(&session) {
                    return None;
                }
                Some(Self::session_snapshot(
                    &session,
                    entry.value_size_bytes,
                    entry.created_at_unix,
                ))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(entries)
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        let Some(entry) = self.kv.reveal_json_entry_for_inspection(entry_ref).await else {
            return Ok(None);
        };
        let session = serde_json::from_value::<SessionRecord>(entry.value.clone())?;
        if is_session_expired(&session) {
            return Ok(None);
        }
        let metadata =
            Self::session_snapshot(&session, entry.value_size_bytes, entry.created_at_unix);
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            metadata,
            entry.value,
            reveal_mode,
        )))
    }
}
