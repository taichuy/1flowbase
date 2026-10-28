use super::*;

pub(super) const JOINS: &str = "LEFT JOIN communication_inputs m ON m.id=t.input_id AND m.scope_id=t.scope_id AND m.instance_id=t.instance_id AND m.external_identity=t.external_identity AND m.conversation_id=t.conversation_id LEFT JOIN communication_conversations c ON c.id=t.conversation_id AND c.scope_id=t.scope_id AND c.instance_id=t.instance_id AND c.external_identity=t.external_identity LEFT JOIN notification_instances n ON n.id=t.instance_id AND n.workspace_id=t.scope_id LEFT JOIN communication_reply_turns rt ON rt.id=m.turn_id AND rt.scope_id=m.scope_id AND rt.instance_id=m.instance_id AND rt.external_identity=m.external_identity AND rt.conversation_id=m.conversation_id LEFT JOIN flow_runs r ON r.id=COALESCE(rt.run_id,m.run_id) AND r.scope_id=t.scope_id";
pub(super) fn columns() -> String {
    format!("t.*,n.display_name AS instance_display_name,c.sender_display_name,COALESCE(rt.run_id,m.run_id) AS run_id,r.application_id,m.processing_status,m.send_status,m.failure_code AS input_failure_code,COALESCE(({RETRY_ELIGIBILITY}),false) AS can_retry")
}
pub(super) fn message(row: PgRow) -> Result<CommunicationMessage> {
    let id: Uuid = row.try_get("id")?;
    let instance_id: Uuid = row.try_get("instance_id")?;
    let kind: String = row.try_get("type")?;
    let file = if kind == "file" {
        Some(CommunicationMessageFile {
            file_name: row.try_get("file_name")?,
            media_type: row.try_get("media_type")?,
            size_bytes: row.try_get("size_bytes")?,
            status: serde_json::from_value(Value::String(row.try_get("status")?))?,
            file_table_id: row.try_get("file_table_id")?,
            file_record_id: row.try_get("file_record_id")?,
            failure_code: row.try_get("failure_code")?,
            access_path: format!("/api/console/communications/instances/{instance_id}/files/{id}"),
        })
    } else {
        None
    };
    Ok(CommunicationMessage {
        id,
        instance_id,
        input_id: row.try_get("input_id")?,
        conversation_id: row.try_get("conversation_id")?,
        instance_display_name: row.try_get("instance_display_name")?,
        external_identity: row.try_get("external_identity")?,
        sender_display_name: row.try_get("sender_display_name")?,
        message_type: serde_json::from_value(Value::String(kind))?,
        direction: serde_json::from_value(Value::String(row.try_get("direction")?))?,
        occurred_at_ms: row.try_get("occurred_at_ms")?,
        content: row.try_get("content")?,
        file,
        run_id: row.try_get("run_id")?,
        application_id: row.try_get("application_id")?,
        processing_status: row
            .try_get::<Option<String>, _>("processing_status")?
            .map(|value| serde_json::from_value(Value::String(value)))
            .transpose()?,
        send_status: row
            .try_get::<Option<String>, _>("send_status")?
            .map(|value| serde_json::from_value(Value::String(value)))
            .transpose()?,
        failure_code: row.try_get("input_failure_code")?,
        can_retry: row.try_get("can_retry")?,
    })
}
pub(super) async fn conversations(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    e: Option<&str>,
    c: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationConversation>> {
    let rows=sqlx::query("SELECT * FROM communication_conversations WHERE scope_id=$1 AND instance_id=$2 AND ($3::text IS NULL OR external_identity=$3) AND ($4::uuid IS NULL OR id<$4) ORDER BY id DESC LIMIT $5")
        .bind(w).bind(i).bind(e).bind(c).bind(i64::from(l.clamp(1,100))+1).fetch_all(store.pool()).await?;
    Ok(page(rows.into_iter().map(conversation).collect(), l, |v| {
        v.id
    }))
}
pub(super) async fn timeline(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Option<Uuid>,
    e: Option<&str>,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
    files: bool,
) -> Result<CommunicationPage<CommunicationMessage>> {
    let limit = l.clamp(1, 5);
    let anchor: Option<i64> = if let Some(id) = p {
        Some(sqlx::query_scalar("SELECT occurred_at_ms FROM communication_messages WHERE scope_id=$1 AND ($2::uuid IS NULL OR instance_id=$2) AND ($3::text IS NULL OR external_identity=$3) AND conversation_id=$4 AND id=$5 AND (NOT $6 OR type='file')")
            .bind(w).bind(i).bind(e).bind(c).bind(id).bind(files).fetch_optional(store.pool()).await?.ok_or(Error::InvalidInput("communication_cursor"))?)
    } else {
        None
    };
    let sql=format!("SELECT {} FROM communication_messages t {JOINS} WHERE t.scope_id=$1 AND ($2::uuid IS NULL OR t.instance_id=$2) AND ($3::text IS NULL OR t.external_identity=$3) AND t.conversation_id=$4 AND ($5::uuid IS NULL OR (t.occurred_at_ms,t.id)<($6,$5)) AND (NOT $7 OR t.type='file') ORDER BY t.occurred_at_ms DESC,t.id DESC LIMIT $8",columns());
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(e)
        .bind(c)
        .bind(p)
        .bind(anchor)
        .bind(files)
        .bind(i64::from(limit) + 1)
        .fetch_all(store.pool())
        .await?;
    let mut result = page(
        rows.into_iter().map(message).collect::<Result<Vec<_>>>()?,
        limit,
        |v| v.id,
    );
    result.items.reverse();
    Ok(result)
}
pub(super) async fn messages(
    store: &PgControlPlaneStore,
    s: &CommunicationHistoryScope,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationMessage>> {
    timeline(
        store,
        s.workspace_id,
        Some(s.instance_id),
        Some(&s.external_identity),
        c,
        p,
        l,
        false,
    )
    .await
}
pub(super) async fn files(
    store: &PgControlPlaneStore,
    s: &CommunicationHistoryScope,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationMessage>> {
    timeline(
        store,
        s.workspace_id,
        Some(s.instance_id),
        Some(&s.external_identity),
        c,
        p,
        l,
        true,
    )
    .await
}
pub(super) async fn file(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    e: Option<&str>,
    id: Uuid,
) -> Result<Option<CommunicationMessage>> {
    let sql=format!("SELECT {} FROM communication_messages t {JOINS} WHERE t.scope_id=$1 AND t.instance_id=$2 AND ($3::text IS NULL OR t.external_identity=$3) AND t.id=$4 AND t.type='file'",columns());
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(e)
        .bind(id)
        .fetch_optional(store.pool())
        .await?
        .map(message)
        .transpose()
}
