use crate::{
    secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad},
    PgControlPlaneStore,
};
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{ports::*, ControlPlaneContractError as Error};
use domain::*;
use serde_json::Value;
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;
// Shared by history projection and the atomic retry write to prevent UI eligibility drift.
const RETRY_ELIGIBILITY: &str = "m.send_status<>'unknown' AND (m.processing_status='failed' OR m.send_status='failed') AND (m.claim_id IS NULL OR m.claim_expires_at<=now()) AND EXISTS(SELECT 1 FROM notification_instances n WHERE n.id=m.instance_id AND n.workspace_id=m.workspace_id AND n.status='enabled')";
mod history;
mod processing;
mod receive;

fn binding(row: PgRow) -> Result<CommunicationBindingRecord> {
    Ok(CommunicationBindingRecord {
        workspace_id: row.get("workspace_id"),
        instance_id: row.get("instance_id"),
        actor_user_id: row.get("actor_user_id"),
        config: serde_json::from_value(row.get("config_json"))?,
        plugin_mcp_instance_id: row.get("plugin_mcp_instance_id"),
        revision: row.get("revision"),
    })
}
fn conversation(row: PgRow) -> CommunicationConversation {
    CommunicationConversation {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        instance_id: row.get("instance_id"),
        external_identity: row.get("external_identity"),
        external_conversation_key: row.get("external_conversation_key"),
        sender_display_name: row.get("sender_display_name"),
    }
}
fn message(row: PgRow) -> Result<CommunicationMessage> {
    Ok(CommunicationMessage {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        external_message_key: row.get("external_message_key"),
        received_at_ms: row.get("received_at_ms"),
        content: row.get("content"),
        can_retry: row.get("can_retry"),
        processing_status: serde_json::from_value(Value::String(row.get("processing_status")))?,
        run_id: row.get("run_id"),
        final_reply: row.get("final_reply"),
        send_status: serde_json::from_value(Value::String(row.get("send_status")))?,
        failure_code: row.get("failure_code"),
    })
}
fn attachment(row: &PgRow) -> Result<CommunicationAttachment> {
    Ok(CommunicationAttachment {
        id: row.get("id"),
        message_id: row.get("message_id"),
        provider_attachment_id: row.get("provider_attachment_id"),
        file_name: row.get("file_name"),
        media_type: row.get("media_type"),
        size_bytes: row.get("size_bytes"),
        status: serde_json::from_value(Value::String(row.get("status")))?,
        file_table_id: row.get("file_table_id"),
        file_record_id: row.get("file_record_id"),
        failure_code: row.get("failure_code"),
    })
}
fn private_aad(workspace: Uuid, instance: Uuid, kind: &str) -> Vec<u8> {
    format!("communication:{workspace}:{instance}:{kind}").into_bytes()
}
fn page<T>(mut items: Vec<T>, limit: u32, id: impl Fn(&T) -> Uuid) -> CommunicationPage<T> {
    let limit = limit.clamp(1, 100) as usize;
    let more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = more.then(|| items.last().map(id)).flatten();
    CommunicationPage { items, next_cursor }
}

#[async_trait]
impl CommunicationRepository for PgControlPlaneStore {
    async fn communication_context_for_run(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<(CommunicationHistoryScope, CommunicationBindingRecord)>> {
        let row=sqlx::query("SELECT b.*,c.external_identity FROM communication_messages m JOIN communication_conversations c ON c.id=m.conversation_id AND c.workspace_id=m.workspace_id AND c.instance_id=m.instance_id JOIN communication_bindings b ON b.instance_id=m.instance_id AND b.workspace_id=m.workspace_id WHERE m.workspace_id=$1 AND m.run_id=$2")
            .bind(workspace_id).bind(run_id).fetch_optional(self.pool()).await?;
        row.map(|row| {
            let scope = CommunicationHistoryScope {
                workspace_id,
                instance_id: row.get("instance_id"),
                external_identity: row.get("external_identity"),
            };
            Ok((scope, binding(row)?))
        })
        .transpose()
    }

    async fn save_communication_binding(
        &self,
        record: CommunicationBindingRecord,
    ) -> Result<CommunicationBindingRecord> {
        let mut tx = self.pool().begin().await?;
        let exists: Option<Uuid> = sqlx::query_scalar("UPDATE notification_instances SET authorization_revision=authorization_revision+1 WHERE workspace_id=$1 AND id=$2 AND type_code <> 'system' RETURNING id")
            .bind(record.workspace_id).bind(record.instance_id).fetch_optional(&mut *tx).await?;
        if exists.is_none() {
            return Err(Error::NotFound("communication_channel").into());
        }
        let result = sqlx::query("INSERT INTO communication_bindings (workspace_id,instance_id,actor_user_id,config_json,plugin_mcp_instance_id) VALUES ($1,$2,$3,$4,$5) ON CONFLICT(instance_id) DO UPDATE SET config_json=excluded.config_json,plugin_mcp_instance_id=excluded.plugin_mcp_instance_id,revision=communication_bindings.revision+1 WHERE communication_bindings.workspace_id=excluded.workspace_id RETURNING *")
            .bind(record.workspace_id).bind(record.instance_id).bind(record.actor_user_id).bind(serde_json::to_value(record.config)?).bind(record.plugin_mcp_instance_id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        binding(result)
    }
    async fn communication_binding(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<CommunicationBindingRecord>> {
        sqlx::query("SELECT * FROM communication_bindings WHERE workspace_id=$1 AND instance_id=$2")
            .bind(workspace_id)
            .bind(instance_id)
            .fetch_optional(self.pool())
            .await?
            .map(binding)
            .transpose()
    }
    async fn enabled_communication_bindings(&self) -> Result<Vec<CommunicationBindingRecord>> {
        sqlx::query("SELECT b.* FROM communication_bindings b JOIN notification_instances n ON n.id=b.instance_id AND n.workspace_id=b.workspace_id WHERE n.status='enabled' ORDER BY b.instance_id").fetch_all(self.pool()).await?.into_iter().map(binding).collect()
    }
    async fn communication_receive_cursor(
        &self,
        workspace: Uuid,
        instance: Uuid,
        master_key: &str,
    ) -> Result<Option<Value>> {
        let value:Option<Value> = sqlx::query_scalar("SELECT encrypted_cursor FROM communication_bindings WHERE workspace_id=$1 AND instance_id=$2").bind(workspace).bind(instance).fetch_optional(self.pool()).await?.flatten();
        value
            .map(|v| {
                decrypt_secret_json_with_aad(
                    &v,
                    master_key,
                    &private_aad(workspace, instance, "cursor"),
                )
            })
            .transpose()
    }
    async fn commit_communication_receive(
        &self,
        input: CommitCommunicationReceiveInput,
    ) -> Result<Option<i64>> {
        receive::commit(self, input).await
    }
    async fn communication_conversations(
        &self,
        w: Uuid,
        i: Uuid,
        e: Option<&str>,
        c: Option<Uuid>,
        l: u32,
    ) -> Result<CommunicationPage<CommunicationConversation>> {
        history::conversations(self, w, i, e, c, l).await
    }
    async fn communication_conversation(
        &self,
        w: Uuid,
        i: Uuid,
        c: Uuid,
    ) -> Result<Option<CommunicationConversation>> {
        Ok(sqlx::query("SELECT * FROM communication_conversations WHERE workspace_id=$1 AND instance_id=$2 AND id=$3").bind(w).bind(i).bind(c).fetch_optional(self.pool()).await?.map(conversation))
    }
    async fn communication_file(
        &self,
        w: Uuid,
        i: Uuid,
        e: Option<&str>,
        a: Uuid,
    ) -> Result<Option<CommunicationAttachment>> {
        sqlx::query("SELECT a.* FROM communication_attachments a JOIN communication_messages m ON m.id=a.message_id JOIN communication_conversations c ON c.id=m.conversation_id WHERE c.workspace_id=$1 AND c.instance_id=$2 AND ($3::text IS NULL OR c.external_identity=$3) AND a.id=$4").bind(w).bind(i).bind(e).bind(a).fetch_optional(self.pool()).await?.as_ref().map(attachment).transpose()
    }
    async fn communication_messages(
        &self,
        s: &CommunicationHistoryScope,
        c: Uuid,
        p: Option<Uuid>,
        l: u32,
    ) -> Result<CommunicationPage<CommunicationMessage>> {
        history::messages(self, s, c, p, l).await
    }
    async fn communication_files(
        &self,
        s: &CommunicationHistoryScope,
        c: Uuid,
        p: Option<Uuid>,
        l: u32,
    ) -> Result<CommunicationPage<CommunicationAttachment>> {
        history::files(self, s, c, p, l).await
    }
    async fn claim_communication_input(
        &self,
        w: Uuid,
        i: Uuid,
        l: u32,
        k: &str,
    ) -> Result<Option<ClaimedCommunicationInput>> {
        processing::claim(self, w, i, l, k).await
    }
    async fn renew_communication_claim(&self, c: &CommunicationClaim, l: u32) -> Result<bool> {
        processing::renew(self, c, l).await
    }
    async fn record_communication_attachment(
        &self,
        c: &CommunicationClaim,
        u: CommunicationAttachmentUpdate,
    ) -> Result<bool> {
        processing::record_attachment(self, c, u).await
    }
    async fn record_communication_progress(
        &self,
        c: &CommunicationClaim,
        p: CommunicationProgress,
    ) -> Result<bool> {
        processing::progress(self, c, p).await
    }
    async fn retry_communication_message(&self, w: Uuid, i: Uuid, m: Uuid) -> Result<bool> {
        processing::retry(self, w, i, m).await
    }
}
