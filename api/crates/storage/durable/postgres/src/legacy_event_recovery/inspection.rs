use super::*;
use control_plane_contracts::ports::{
    EphemeralEntrySnapshot, EphemeralEntryValueSnapshot, EphemeralEntryValueState,
    EphemeralValueRevealMode, ephemeral_metadata_size_bytes,
};

impl PgLegacyEventRecovery {
    fn metadata(row: &sqlx::postgres::PgRow) -> Result<EphemeralEntrySnapshot> {
        let topic = String::from_utf8(row.try_get::<Vec<u8>, _>("topic")?)?;
        let sequence: i64 = row.try_get("sequence")?;
        let entry_ref = sequence.to_string();
        let metadata = serde_json::json!({
            "topic": topic, "sequence": sequence,
            "storage_medium": "postgresql-durable", "size_basis": "serialized-json",
        });
        Ok(EphemeralEntrySnapshot {
            contract_code: "event-bus".into(),
            group_code: Some(topic.clone()),
            entry_ref: entry_ref.clone(),
            key: format!("{topic}:{sequence}"),
            inspection_path: vec![topic, entry_ref],
            entry_kind: "event".into(),
            status: "buffered".into(),
            owner: None,
            value_size_bytes: row.try_get::<i64, _>("value_size_bytes")? as u64,
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: None,
            created_at_unix: None,
            expires_at_unix: None,
            sensitive: true,
            metadata,
        })
    }

    pub(super) async fn list_metadata(&self) -> Result<Vec<EphemeralEntrySnapshot>> {
        // Project scalar metadata only. Never load all pending JSON to serve inspection.
        // The legacy Vec contract still has O(entry count) transient metadata cost.
        let rows = sqlx::query(
            "select sequence,topic,value_size_bytes from legacy_event_recovery_ledger where disposition='pending'",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entries = rows
            .iter()
            .map(Self::metadata)
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by(|a, b| a.group_code.cmp(&b.group_code).then(a.key.cmp(&b.key)));
        Ok(entries)
    }

    pub(super) async fn reveal(
        &self,
        entry_ref: &str,
        mode: EphemeralValueRevealMode,
    ) -> Result<Option<EphemeralEntryValueSnapshot>> {
        let Ok(sequence) = entry_ref.parse::<i64>() else {
            return Ok(None);
        };
        let mut tx = self.pool.begin().await?;
        // A stable read snapshot permits a racing poll without mixing different entries.
        sqlx::query("set transaction isolation level repeatable read read only")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "select sequence,topic,value_size_bytes from legacy_event_recovery_ledger where disposition='pending' and sequence=$1",
        )
        .bind(sequence)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let metadata = Self::metadata(&row)?;
        if matches!(mode, EphemeralValueRevealMode::Metadata) {
            let size = metadata.value_size_bytes;
            tx.commit().await?;
            return Ok(Some(EphemeralEntryValueSnapshot {
                metadata,
                reveal_mode: mode,
                value_state: EphemeralEntryValueState::Hidden,
                value: None,
                value_preview: None,
                preview_size_bytes: 0,
                full_value_size_bytes: size,
            }));
        }
        let row =
            sqlx::query("select payload from legacy_event_recovery_ledger where disposition='pending' and sequence=$1")
                .bind(sequence)
                .fetch_one(&mut *tx)
                .await?;
        let payload = Self::payload(&row)?;
        tx.commit().await?;
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            metadata, payload, mode,
        )))
    }
}
