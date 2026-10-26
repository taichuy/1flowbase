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
const RETRY_ELIGIBILITY: &str = "m.send_status<>'unknown' AND (m.processing_status='failed' OR m.send_status='failed') AND (m.claim_id IS NULL OR m.claim_expires_at<=now()) AND EXISTS(SELECT 1 FROM notification_instances n WHERE n.id=m.instance_id AND n.workspace_id=m.scope_id AND n.status='enabled')";
mod history;
mod management;
mod processing;
mod receive;

fn binding(row: PgRow) -> Result<CommunicationBindingRecord> {
    Ok(CommunicationBindingRecord {
        workspace_id: row.get("scope_id"),
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
        workspace_id: row.get("scope_id"),
        instance_id: row.get("instance_id"),
        external_identity: row.get("external_identity"),
        external_conversation_key: row.get("external_conversation_key"),
        sender_display_name: row.get("sender_display_name"),
    }
}
fn input(row: PgRow) -> Result<CommunicationInput> {
    Ok(CommunicationInput {
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
        message_id: row.get("input_id"),
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
    async fn communication_management_history(
        &self,
        w: Uuid,
        c: Uuid,
        p: Option<Uuid>,
        l: u32,
    ) -> Result<CommunicationPage<CommunicationMessage>> {
        history::timeline(self, w, None, None, c, p, l, false).await
    }
    async fn communication_management_conversations(
        &self,
        workspace_id: Uuid,
        query: &CommunicationManagementQuery,
    ) -> Result<CommunicationManagementPage<CommunicationConversationListItem>> {
        management::conversations(self, workspace_id, query).await
    }
    async fn communication_management_messages(
        &self,
        workspace_id: Uuid,
        query: &CommunicationManagementQuery,
    ) -> Result<CommunicationManagementPage<CommunicationMessageListItem>> {
        management::messages(self, workspace_id, query).await
    }
    async fn communication_management_message(
        &self,
        workspace_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<CommunicationMessageDetail>> {
        management::detail(self, workspace_id, message_id).await
    }

    async fn communication_run_application_id(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>> {
        Ok(sqlx::query_scalar("SELECT r.application_id FROM communication_inputs m JOIN flow_runs r ON r.id=m.run_id AND r.scope_id=m.scope_id WHERE m.scope_id=$1 AND m.run_id=$2").bind(workspace_id).bind(run_id).fetch_optional(self.pool()).await?)
    }

    async fn communication_context_for_run(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<(CommunicationHistoryScope, CommunicationBindingRecord)>> {
        let row=sqlx::query("SELECT b.*,m.external_identity FROM communication_inputs m JOIN communication_bindings b ON b.instance_id=m.instance_id AND b.scope_id=m.scope_id WHERE m.scope_id=$1 AND m.run_id=$2")
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
        let result = sqlx::query("INSERT INTO communication_bindings (scope_id,instance_id,actor_user_id,config_json,plugin_mcp_instance_id) VALUES ($1,$2,$3,$4,$5) ON CONFLICT(instance_id) DO UPDATE SET config_json=excluded.config_json,plugin_mcp_instance_id=excluded.plugin_mcp_instance_id,revision=communication_bindings.revision+1 WHERE communication_bindings.scope_id=excluded.scope_id RETURNING *")
            .bind(record.workspace_id).bind(record.instance_id).bind(record.actor_user_id).bind(serde_json::to_value(record.config)?).bind(record.plugin_mcp_instance_id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        binding(result)
    }
    async fn communication_binding(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<CommunicationBindingRecord>> {
        sqlx::query("SELECT * FROM communication_bindings WHERE scope_id=$1 AND instance_id=$2")
            .bind(workspace_id)
            .bind(instance_id)
            .fetch_optional(self.pool())
            .await?
            .map(binding)
            .transpose()
    }
    async fn enabled_communication_bindings(&self) -> Result<Vec<CommunicationBindingRecord>> {
        sqlx::query("SELECT b.* FROM communication_bindings b JOIN notification_instances n ON n.id=b.instance_id AND n.workspace_id=b.scope_id WHERE n.status='enabled' ORDER BY b.instance_id").fetch_all(self.pool()).await?.into_iter().map(binding).collect()
    }
    async fn communication_receive_cursor(
        &self,
        workspace: Uuid,
        instance: Uuid,
        master_key: &str,
    ) -> Result<Option<Value>> {
        let value:Option<Value> = sqlx::query_scalar("SELECT encrypted_cursor FROM communication_bindings WHERE scope_id=$1 AND instance_id=$2").bind(workspace).bind(instance).fetch_optional(self.pool()).await?.flatten();
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
        Ok(sqlx::query("SELECT id,scope_id,instance_id,external_identity,external_conversation_key,sender_display_name FROM communication_conversations WHERE scope_id=$1 AND instance_id=$2 AND id=$3 UNION ALL SELECT conversation_id,scope_id,instance_id,external_identity,'' AS external_conversation_key,NULL AS sender_display_name FROM communication_messages WHERE scope_id=$1 AND instance_id=$2 AND conversation_id=$3 LIMIT 1").bind(w).bind(i).bind(c).fetch_optional(self.pool()).await?.map(conversation))
    }
    async fn communication_file(
        &self,
        w: Uuid,
        i: Uuid,
        e: Option<&str>,
        a: Uuid,
    ) -> Result<Option<CommunicationMessage>> {
        history::file(self, w, i, e, a).await
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
    ) -> Result<CommunicationPage<CommunicationMessage>> {
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
        let input_id: Option<Uuid> = sqlx::query_scalar("SELECT input_id FROM communication_messages WHERE scope_id=$1 AND instance_id=$2 AND id=$3").bind(w).bind(i).bind(m).fetch_optional(self.pool()).await?;
        match input_id {
            Some(id) => processing::retry(self, w, i, id).await,
            None => Ok(false),
        }
    }
}
