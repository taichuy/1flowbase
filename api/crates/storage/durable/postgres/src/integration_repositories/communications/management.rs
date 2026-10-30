//! Workspace management projections; delegated history uses a separately enforced scope.
use super::*;
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
    let sql = format!(
        "SELECT c.*,n.display_name AS instance_display_name FROM communication_conversations c LEFT JOIN notification_instances n ON n.id=c.instance_id AND n.workspace_id=c.scope_id WHERE {filter} ORDER BY c.id DESC LIMIT $4 OFFSET $5"
    );
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
            Ok(CommunicationConversationListItem {
                instance_display_name: row.try_get("instance_display_name")?,
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
pub(super) async fn messages(
    store: &PgControlPlaneStore,
    w: Uuid,
    q: &CommunicationManagementQuery,
) -> Result<CommunicationManagementPage<CommunicationMessageListItem>> {
    let (page, page_size, offset) = pagination(q);
    let processing = status(&q.processing_status)?;
    let send = status(&q.send_status)?;
    let direction = status(&q.direction)?;
    let kind = status(&q.message_type)?;
    let filter = "t.scope_id=$1 AND ($2::uuid IS NULL OR t.instance_id=$2) AND ($3::uuid IS NULL OR t.conversation_id=$3) AND ($4::text IS NULL OR m.processing_status=$4) AND ($5::text IS NULL OR m.send_status=$5) AND ($6::text IS NULL OR t.direction=$6) AND ($7::text IS NULL OR t.type=$7)";
    let sql = format!(
        "SELECT count(*) FROM communication_messages t {} WHERE {filter}",
        history::JOINS
    );
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .bind(&processing)
        .bind(&send)
        .bind(&direction)
        .bind(&kind)
        .fetch_one(store.pool())
        .await?;
    let sql = format!(
        "SELECT {} FROM communication_messages t {} WHERE {filter} ORDER BY t.occurred_at_ms DESC,t.id DESC LIMIT $8 OFFSET $9",
        history::columns(),
        history::JOINS
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(q.instance_id)
        .bind(q.conversation_id)
        .bind(processing)
        .bind(send)
        .bind(direction)
        .bind(kind)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(store.pool())
        .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            let mut message = history::message(row)?;
            let content_preview = message
                .content
                .take()
                .map(|v| v.chars().take(160).collect());
            Ok(CommunicationMessageListItem {
                message,
                content_preview,
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
    let sql = format!(
        "SELECT {} FROM communication_messages t {} WHERE t.scope_id=$1 AND t.id=$2",
        history::columns(),
        history::JOINS
    );
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(id)
        .fetch_optional(store.pool())
        .await?
        .map(history::message)
        .transpose()
}
