use super::*;

pub const MCP_RESULT_RECEIPT_SUMMARY_MAX_BYTES: usize = 4096;
pub const MCP_RESULT_RECEIPT_OPERATION_ID_MAX_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOperationOutcome {
    Succeeded,
    Failed,
}

impl McpOperationOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordMcpResultReceiptInput {
    pub receipt_id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub operation_id: String,
    pub outcome: McpOperationOutcome,
    pub summary: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpResultReceipt {
    pub receipt_id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub operation_id: String,
    pub outcome: McpOperationOutcome,
    pub summary: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[async_trait]
pub trait McpResultReceiptRepository: Send + Sync {
    /// Records a compact terminal outcome. Reusing a receipt ID returns the first durable record;
    /// it never replaces the original outcome or summary.
    async fn record_mcp_result_receipt(
        &self,
        input: &RecordMcpResultReceiptInput,
    ) -> anyhow::Result<McpResultReceipt>;

    async fn get_mcp_result_receipt(
        &self,
        workspace_id: Uuid,
        receipt_id: Uuid,
    ) -> anyhow::Result<Option<McpResultReceipt>>;
}
