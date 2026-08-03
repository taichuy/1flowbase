use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane::ports::{
    McpOperationOutcome, McpResultReceipt, McpResultReceiptRepository, RecordMcpResultReceiptInput,
    MCP_RESULT_RECEIPT_OPERATION_ID_MAX_BYTES, MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES,
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

const RECEIPT_TARGET_TYPE: &str = "mcp_result_receipt";
const RECEIPT_EVENT_CODE: &str = "mcp.operation.completed";

#[async_trait]
impl McpResultReceiptRepository for PgControlPlaneStore {
    async fn record_mcp_result_receipt(
        &self,
        input: &RecordMcpResultReceiptInput,
    ) -> Result<McpResultReceipt> {
        if input.operation_id.is_empty()
            || input.operation_id.len() > MCP_RESULT_RECEIPT_OPERATION_ID_MAX_BYTES
        {
            bail!(
                "MCP result receipt operation_id must contain 1 to {} bytes",
                MCP_RESULT_RECEIPT_OPERATION_ID_MAX_BYTES
            );
        }
        let summary_bytes = serde_json::to_vec(&input.summary)?;
        if summary_bytes.len() > MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES {
            bail!(
                "MCP result receipt summary exceeds {} bytes",
                MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES
            );
        }

        sqlx::query(
            r#"
            insert into audit_logs (
                id, workspace_id, actor_user_id, target_type, target_id, event_code, payload
            ) values (
                $1, $2, $3, $4, $1, $5,
                jsonb_build_object(
                    'operation_id', $6::text,
                    'outcome', $7::text,
                    'summary', $8::jsonb
                )
            )
            on conflict (id) do nothing
            "#,
        )
        .bind(input.receipt_id)
        .bind(input.workspace_id)
        .bind(input.actor_user_id)
        .bind(RECEIPT_TARGET_TYPE)
        .bind(RECEIPT_EVENT_CODE)
        .bind(&input.operation_id)
        .bind(input.outcome.as_str())
        .bind(&input.summary)
        .execute(self.pool())
        .await?;

        self.get_mcp_result_receipt(input.workspace_id, input.receipt_id)
            .await?
            .ok_or_else(|| anyhow!("MCP result receipt ID conflicts with another durable record"))
    }

    async fn get_mcp_result_receipt(
        &self,
        workspace_id: Uuid,
        receipt_id: Uuid,
    ) -> Result<Option<McpResultReceipt>> {
        let row = sqlx::query(
            r#"
            select id, workspace_id, actor_user_id, payload, created_at
            from audit_logs
            where workspace_id = $1
              and id = $2
              and target_type = $3
              and event_code = $4
            "#,
        )
        .bind(workspace_id)
        .bind(receipt_id)
        .bind(RECEIPT_TARGET_TYPE)
        .bind(RECEIPT_EVENT_CODE)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_receipt).transpose()
    }
}

fn map_receipt(row: sqlx::postgres::PgRow) -> Result<McpResultReceipt> {
    let payload: serde_json::Value = row.try_get("payload")?;
    let operation_id = payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("MCP result receipt is missing operation_id"))?;
    let outcome = match payload.get("outcome").and_then(serde_json::Value::as_str) {
        Some("succeeded") => McpOperationOutcome::Succeeded,
        Some("failed") => McpOperationOutcome::Failed,
        _ => bail!("MCP result receipt has an invalid outcome"),
    };
    let summary = payload
        .get("summary")
        .cloned()
        .ok_or_else(|| anyhow!("MCP result receipt is missing summary"))?;

    Ok(McpResultReceipt {
        receipt_id: row.try_get("id")?,
        workspace_id: row.try_get("workspace_id")?,
        actor_user_id: row.try_get("actor_user_id")?,
        operation_id: operation_id.to_string(),
        outcome,
        summary,
        created_at: row.try_get("created_at")?,
    })
}
