use async_trait::async_trait;
use control_plane_contracts::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolCapsuleStore,
    ProviderProtocolContextSlotId, ProviderProtocolContextValue, ProviderTransportAffinity,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad};

const KEY_VERSION: &str = "provider-secret-master-key:v1";

#[derive(Clone)]
pub struct PgProviderProtocolCapsuleStore {
    pool: PgPool,
    master_key: String,
    hard_retention: Duration,
    max_plaintext_bytes: usize,
}

impl PgProviderProtocolCapsuleStore {
    fn associated_data(flow_run_id: Uuid, kind: &str, slot_key: &str) -> Vec<u8> {
        format!("provider_protocol_capsule:v1\0{flow_run_id}\0{kind}\0{slot_key}").into_bytes()
    }

    pub fn new(
        pool: PgPool,
        master_key: impl Into<String>,
        hard_retention: Duration,
        max_plaintext_bytes: usize,
    ) -> anyhow::Result<Self> {
        let master_key = master_key.into();
        anyhow::ensure!(
            !master_key.is_empty() && hard_retention > Duration::ZERO && max_plaintext_bytes > 0,
            "provider_protocol_capsule_policy_invalid"
        );
        Ok(Self {
            pool,
            master_key,
            hard_retention,
            max_plaintext_bytes,
        })
    }

    async fn put_value(
        &self,
        flow_run_id: Uuid,
        kind: &'static str,
        slot_key: String,
        value: Value,
    ) -> anyhow::Result<()> {
        let plaintext = serde_json::to_vec(&value)?;
        anyhow::ensure!(
            !plaintext.is_empty() && plaintext.len() <= self.max_plaintext_bytes,
            "provider_protocol_capsule_too_large"
        );
        let digest = format!("sha256:{:x}", Sha256::digest(&plaintext));
        let associated_data = Self::associated_data(flow_run_id, kind, &slot_key);
        let encrypted = encrypt_secret_json_with_aad(&value, &self.master_key, &associated_data)?;
        let hard_expires_at = OffsetDateTime::now_utc() + self.hard_retention;
        sqlx::query(
            r#"
            insert into provider_protocol_capsules (
                flow_run_id, capsule_kind, slot_key, encrypted_payload,
                plaintext_digest, plaintext_size_bytes, key_version, hard_expires_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (flow_run_id, capsule_kind, slot_key) do update
            set encrypted_payload = excluded.encrypted_payload,
                plaintext_digest = excluded.plaintext_digest,
                plaintext_size_bytes = excluded.plaintext_size_bytes,
                key_version = excluded.key_version,
                hard_expires_at = excluded.hard_expires_at,
                updated_at = now()
            "#,
        )
        .bind(flow_run_id)
        .bind(kind)
        .bind(&slot_key)
        .bind(encrypted)
        .bind(digest)
        .bind(i64::try_from(plaintext.len())?)
        .bind(KEY_VERSION)
        .bind(hard_expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_value(
        &self,
        flow_run_id: Uuid,
        kind: &'static str,
        slot_key: String,
    ) -> anyhow::Result<Option<Value>> {
        let row = sqlx::query(
            r#"
            delete from provider_protocol_capsules
            where flow_run_id = $1 and capsule_kind = $2 and slot_key = $3
              and hard_expires_at <= now()
            "#,
        )
        .bind(flow_run_id)
        .bind(kind)
        .bind(&slot_key)
        .execute(&self.pool)
        .await?;
        if row.rows_affected() > 0 {
            return Ok(None);
        }
        let row = sqlx::query(
            r#"
            select encrypted_payload, plaintext_digest, plaintext_size_bytes, key_version
            from provider_protocol_capsules
            where flow_run_id = $1 and capsule_kind = $2 and slot_key = $3
              and hard_expires_at > now()
            "#,
        )
        .bind(flow_run_id)
        .bind(kind)
        .bind(&slot_key)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        anyhow::ensure!(
            row.try_get::<String, _>("key_version")? == KEY_VERSION,
            "provider_protocol_capsule_key_version_unsupported"
        );
        let encrypted: Value = row.try_get("encrypted_payload")?;
        let associated_data = Self::associated_data(flow_run_id, kind, &slot_key);
        let value = decrypt_secret_json_with_aad(&encrypted, &self.master_key, &associated_data)?;
        let plaintext = serde_json::to_vec(&value)?;
        anyhow::ensure!(
            i64::try_from(plaintext.len())? == row.try_get::<i64, _>("plaintext_size_bytes")?
                && format!("sha256:{:x}", Sha256::digest(&plaintext))
                    == row.try_get::<String, _>("plaintext_digest")?,
            "provider_protocol_capsule_integrity_mismatch"
        );
        Ok(Some(value))
    }

    async fn delete_slot(
        &self,
        flow_run_id: Uuid,
        kind: &'static str,
        slot_key: String,
    ) -> anyhow::Result<bool> {
        Ok(sqlx::query(
            "delete from provider_protocol_capsules where flow_run_id = $1 and capsule_kind = $2 and slot_key = $3",
        )
        .bind(flow_run_id)
        .bind(kind)
        .bind(&slot_key)
        .execute(&self.pool)
        .await?
        .rows_affected()
            > 0)
    }

    async fn take_value(
        &self,
        flow_run_id: Uuid,
        kind: &'static str,
        slot_key: String,
    ) -> anyhow::Result<Value> {
        let row = sqlx::query(
            r#"
            delete from provider_protocol_capsules
            where flow_run_id = $1 and capsule_kind = $2 and slot_key = $3
              and hard_expires_at > now()
            returning encrypted_payload, plaintext_digest, plaintext_size_bytes, key_version
            "#,
        )
        .bind(flow_run_id)
        .bind(kind)
        .bind(&slot_key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("provider_continuation_missing"))?;
        anyhow::ensure!(
            row.try_get::<String, _>("key_version")? == KEY_VERSION,
            "provider_protocol_capsule_key_version_unsupported"
        );
        let associated_data = Self::associated_data(flow_run_id, kind, &slot_key);
        let value = decrypt_secret_json_with_aad(
            &row.try_get::<Value, _>("encrypted_payload")?,
            &self.master_key,
            &associated_data,
        )?;
        let plaintext = serde_json::to_vec(&value)?;
        anyhow::ensure!(
            i64::try_from(plaintext.len())? == row.try_get::<i64, _>("plaintext_size_bytes")?
                && format!("sha256:{:x}", Sha256::digest(&plaintext))
                    == row.try_get::<String, _>("plaintext_digest")?,
            "provider_protocol_capsule_integrity_mismatch"
        );
        Ok(value)
    }

    fn continuation_from_value(value: &Value) -> anyhow::Result<ProviderContinuation> {
        let affinity = value
            .get("affinity")
            .ok_or_else(|| anyhow::anyhow!("provider_protocol_capsule_invalid"))?;
        let field = |name| {
            affinity
                .get(name)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("provider_protocol_capsule_invalid"))
        };
        let affinity = ProviderTransportAffinity::new(
            field("provider_instance_id")?,
            field("provider_code")?,
            field("protocol")?,
            field("model")?,
        );
        let response_id = value
            .get("response_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("provider_protocol_capsule_invalid"))?;
        ProviderContinuation::new(response_id, affinity)
    }
}

#[async_trait]
impl ProviderProtocolCapsuleStore for PgProviderProtocolCapsuleStore {
    async fn put_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
        value: ProviderProtocolContextValue,
    ) -> anyhow::Result<()> {
        self.put_value(
            slot_id.flow_run_id(),
            "protocol_context",
            slot_id.storage_key(),
            value.into_value(),
        )
        .await
    }

    async fn get_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<ProviderProtocolContextValue>> {
        self.get_value(
            slot_id.flow_run_id(),
            "protocol_context",
            slot_id.storage_key(),
        )
        .await?
        .map(ProviderProtocolContextValue::new)
        .transpose()
    }

    async fn delete_flow_run_protocol_contexts(&self, flow_run_id: Uuid) -> anyhow::Result<usize> {
        let affected = sqlx::query(
            "delete from provider_protocol_capsules where flow_run_id = $1 and capsule_kind = 'protocol_context'",
        )
        .bind(flow_run_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(usize::try_from(affected)?)
    }

    async fn put_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<()> {
        let affinity = continuation.affinity();
        let value = json!({
            "response_id": continuation.response_id(),
            "affinity": {
                "provider_instance_id": affinity.provider_instance_id(),
                "provider_code": affinity.provider_code(),
                "protocol": affinity.protocol(),
                "model": affinity.model(),
            }
        });
        self.put_value(
            slot_id.flow_run_id(),
            "continuation",
            slot_id.storage_key(),
            value,
        )
        .await
    }

    async fn get_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<ProviderContinuation>> {
        let Some(value) = self
            .get_value(slot_id.flow_run_id(), "continuation", slot_id.storage_key())
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Self::continuation_from_value(&value)?))
    }

    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation> {
        let value = self
            .take_value(slot_id.flow_run_id(), "continuation", slot_id.storage_key())
            .await?;
        Self::continuation_from_value(&value)
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        self.delete_slot(slot_id.flow_run_id(), "continuation", slot_id.storage_key())
            .await
    }

    async fn clear_expired(&self) -> anyhow::Result<usize> {
        let affected =
            sqlx::query("delete from provider_protocol_capsules where hard_expires_at <= now()")
                .execute(&self.pool)
                .await?
                .rows_affected();
        Ok(usize::try_from(affected)?)
    }
}
