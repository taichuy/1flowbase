//! Workspace management read models. Delegated history retains its separate, mandatory scope.
use super::*;

const MESSAGE_JOINS: &str = "JOIN communication_conversations c ON c.id=m.conversation_id AND c.scope_id=m.scope_id AND c.instance_id=m.instance_id LEFT JOIN notification_instances n ON n.id=m.instance_id AND n.workspace_id=m.scope_id LEFT JOIN flow_runs r ON r.id=m.run_id AND r.scope_id=m.scope_id";
const MESSAGE_FILTER: &str = "m.scope_id=$1 AND ($2::uuid IS NULL OR m.instance_id=$2) AND ($3::uuid IS NULL OR m.conversation_id=$3) AND ($4::text IS NULL OR m.processing_status=$4) AND ($5::text IS NULL OR m.send_status=$5)";
fn summary_columns() -> String {
    format!("m.id,m.conversation_id,m.instance_id,n.display_name AS instance_display_name,m.external_identity,c.sender_display_name,m.received_at_ms,left(m.content,160) AS content_preview,m.processing_status,m.send_status,m.failure_code,({RETRY_ELIGIBILITY}) AS can_retry,m.run_id,r.application_id")
}
fn pagination(q: &CommunicationManagementQuery) -> (u32, u32, i64) {
    let page = q.page.unwrap_or(1).max(1);
    let size = q.page_size.unwrap_or(20).clamp(1, 100);
    (page, size, i64::from(page - 1) * i64::from(size))
}
fn status<T: serde::Serialize>(v: &Option<T>) -> Result<Option<String>> {
    v.as_ref()
        .map(|v| Ok(serde_json::from_value::<String>(serde_json::to_value(v)?)?))
        .transpose()
}
fn summary(row: &PgRow) -> Result<CommunicationMessageListItem> {
    Ok(CommunicationMessageListItem {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        instance_id: row.try_get("instance_id")?,
        instance_display_name: row.try_get("instance_display_name")?,
        external_identity: row.try_get("external_identity")?,
        sender_display_name: row.try_get("sender_display_name")?,
        received_at_ms: row.try_get("received_at_ms")?,
        content_preview: row.try_get("content_preview")?,
        processing_status: serde_json::from_value(Value::String(
            row.try_get("processing_status")?,
        ))?,
        send_status: serde_json::from_value(Value::String(row.try_get("send_status")?))?,
        failure_code: row.try_get("failure_code")?,
        can_retry: row.try_get("can_retry")?,
        run_id: row.try_get("run_id")?,
        application_id: row.try_get("application_id")?,
    })
}
pub(super) async fn messages(
    store: &PgControlPlaneStore,
    w: Uuid,
    q: &CommunicationManagementQuery,
) -> Result<CommunicationManagementPage<CommunicationMessageListItem>> {
    let (page, page_size, offset) = pagination(q);
    let processing = status(&q.processing_status)?;
    let send = status(&q.send_status)?;
    let count_sql = format!("SELECT count(*) FROM communication_messages m WHERE {MESSAGE_FILTER}");
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .bind(&processing)
        .bind(&send)
        .fetch_one(store.pool())
        .await?;
    let sql = format!("SELECT {} FROM communication_messages m {MESSAGE_JOINS} WHERE {MESSAGE_FILTER} ORDER BY m.received_at_ms DESC,m.id DESC LIMIT $6 OFFSET $7", summary_columns());
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .bind(&processing)
        .bind(&send)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(store.pool())
        .await?;
    Ok(CommunicationManagementPage {
        items: rows.iter().map(summary).collect::<Result<_>>()?,
        total,
        page,
        page_size,
    })
}
pub(super) async fn conversations(
    store: &PgControlPlaneStore,
    w: Uuid,
    q: &CommunicationManagementQuery,
) -> Result<CommunicationManagementPage<CommunicationConversationListItem>> {
    let (page, page_size, offset) = pagination(q);
    let filter = "c.scope_id=$1 AND ($2::uuid IS NULL OR c.instance_id=$2) AND ($3::uuid IS NULL OR c.id=$3)";
    let sql = format!("SELECT count(*) FROM communication_conversations c WHERE {filter}");
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .fetch_one(store.pool())
        .await?;
    let sql = format!("SELECT c.*,n.display_name AS instance_display_name,to_jsonb(latest) AS latest_message FROM communication_conversations c LEFT JOIN notification_instances n ON n.id=c.instance_id AND n.workspace_id=c.scope_id LEFT JOIN LATERAL (SELECT {} FROM communication_messages m LEFT JOIN flow_runs r ON r.id=m.run_id AND r.scope_id=m.scope_id WHERE m.conversation_id=c.id AND m.scope_id=c.scope_id AND m.instance_id=c.instance_id ORDER BY m.received_at_ms DESC,m.id DESC LIMIT 1) latest ON true WHERE {filter} ORDER BY latest.received_at_ms DESC NULLS LAST,latest.id DESC NULLS LAST,c.id DESC LIMIT $4 OFFSET $5", summary_columns());
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(store.pool())
        .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            let latest: Option<Value> = row.try_get("latest_message")?;
            Ok(CommunicationConversationListItem {
                instance_display_name: row.try_get("instance_display_name")?,
                latest_message: latest.map(serde_json::from_value).transpose()?,
                conversation: conversation(row),
            })
        })
        .collect::<Result<_>>()?;
    Ok(CommunicationManagementPage {
        items,
        total,
        page,
        page_size,
    })
}
pub(super) async fn detail(
    store: &PgControlPlaneStore,
    w: Uuid,
    id: Uuid,
) -> Result<Option<CommunicationMessageDetail>> {
    let sql = format!("SELECT m.*,n.display_name AS instance_display_name,c.sender_display_name,r.application_id,({RETRY_ELIGIBILITY}) AS can_retry FROM communication_messages m {MESSAGE_JOINS} WHERE m.scope_id=$1 AND m.id=$2");
    let Some(row) = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(id)
        .fetch_optional(store.pool())
        .await?
    else {
        return Ok(None);
    };
    let attachments = sqlx::query(
        "SELECT * FROM communication_attachments WHERE scope_id=$1 AND message_id=$2 ORDER BY id",
    )
    .bind(w)
    .bind(id)
    .fetch_all(store.pool())
    .await?
    .iter()
    .map(attachment)
    .collect::<Result<Vec<_>>>()?;
    Ok(Some(CommunicationMessageDetail {
        instance_id: row.try_get("instance_id")?,
        instance_display_name: row.try_get("instance_display_name")?,
        external_identity: row.try_get("external_identity")?,
        sender_display_name: row.try_get("sender_display_name")?,
        application_id: row.try_get("application_id")?,
        message: message(row)?,
        attachments,
    }))
}
