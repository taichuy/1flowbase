use super::*;

pub(super) const JOINS: &str = "LEFT JOIN communication_inputs m ON m.id=t.input_id AND m.scope_id=t.scope_id AND m.instance_id=t.instance_id AND m.external_identity=t.external_identity AND m.conversation_id=t.conversation_id LEFT JOIN communication_conversations c ON c.id=t.conversation_id AND c.scope_id=t.scope_id AND c.instance_id=t.instance_id AND c.external_identity=t.external_identity LEFT JOIN notification_instances n ON n.id=t.instance_id AND n.workspace_id=t.scope_id LEFT JOIN communication_reply_turns rt ON rt.id=m.turn_id AND rt.scope_id=m.scope_id AND rt.instance_id=m.instance_id AND rt.external_identity=m.external_identity AND rt.conversation_id=m.conversation_id LEFT JOIN LATERAL (SELECT CASE WHEN x.send_status='sending' AND ((x.run_id IS NOT NULL AND xr.status IN ('succeeded','incomplete','failed','cancelled')) OR (x.run_id IS NULL AND COALESCE(rt.send_status,m.send_status) IN ('unknown','failed'))) THEN 'unknown' ELSE x.send_status END AS send_status,CASE WHEN x.send_status='sending' AND ((x.run_id IS NOT NULL AND xr.status IN ('succeeded','incomplete','failed','cancelled')) OR (x.run_id IS NULL AND COALESCE(rt.send_status,m.send_status) IN ('unknown','failed'))) THEN COALESCE(rt.failure_code,m.failure_code,'send_interrupted') ELSE x.failure_code END AS failure_code,x.run_id FROM communication_message_sends x LEFT JOIN flow_runs xr ON xr.id=x.run_id AND xr.scope_id=x.scope_id WHERE x.scope_id=t.scope_id AND x.instance_id=t.instance_id AND x.conversation_id=t.conversation_id AND ((t.direction='outbound' AND x.message_id=t.id) OR (t.direction='inbound' AND x.input_id=t.input_id)) ORDER BY x.created_at DESC,x.send_log_id DESC LIMIT 1) ns ON true LEFT JOIN flow_runs r ON r.id=COALESCE(ns.run_id,rt.run_id,m.run_id) AND r.scope_id=t.scope_id";
pub(super) fn columns() -> String {
    format!(
        "t.*,CASE WHEN t.direction='inbound' THEN m.reply_to_json END AS reply_to_json,n.display_name AS instance_display_name,c.sender_display_name,COALESCE(ns.run_id,rt.run_id,m.run_id) AS run_id,r.application_id,COALESCE(CASE WHEN t.direction='outbound' THEN CASE ns.send_status WHEN 'succeeded' THEN 'succeeded' WHEN 'failed' THEN 'failed' WHEN 'unknown' THEN 'failed' WHEN 'sending' THEN 'processing' END END,m.processing_status) AS processing_status,COALESCE(ns.send_status,m.send_status) AS send_status,COALESCE(ns.failure_code,m.failure_code) AS input_failure_code,(COALESCE(({RETRY_ELIGIBILITY}),false) AND COALESCE(ns.send_status NOT IN ('succeeded','unknown','sending'),true)) AS can_retry"
    )
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
        reply_to: row
            .try_get::<Option<Value>, _>("reply_to_json")?
            .map(serde_json::from_value)
            .transpose()?,
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
pub(super) struct TimelineScope<'a> {
    pub(super) workspace_id: Uuid,
    pub(super) instance_id: Option<Uuid>,
    pub(super) external_identity: Option<&'a str>,
}

pub(super) async fn timeline(
    store: &PgControlPlaneStore,
    scope: TimelineScope<'_>,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
    files: bool,
    keyword: Option<&str>,
) -> Result<CommunicationPage<CommunicationMessage>> {
    let TimelineScope {
        workspace_id: w,
        instance_id: i,
        external_identity: e,
    } = scope;
    let limit = l.clamp(1, 5);
    let anchor: Option<i64> = if let Some(id) = p {
        Some(sqlx::query_scalar("SELECT occurred_at_ms FROM communication_messages WHERE scope_id=$1 AND ($2::uuid IS NULL OR instance_id=$2) AND ($3::text IS NULL OR external_identity=$3) AND conversation_id=$4 AND id=$5 AND (NOT $6 OR type='file')")
            .bind(w).bind(i).bind(e).bind(c).bind(id).bind(files).fetch_optional(store.pool()).await?.ok_or(Error::InvalidInput("communication_cursor"))?)
    } else {
        None
    };
    let sql = format!(
        "SELECT {} FROM communication_messages t {JOINS} WHERE t.scope_id=$1 AND ($2::uuid IS NULL OR t.instance_id=$2) AND ($3::text IS NULL OR t.external_identity=$3) AND t.conversation_id=$4 AND ($5::uuid IS NULL OR (t.occurred_at_ms,t.id)<($6,$5)) AND (NOT $7 OR t.type='file') AND ($9::text IS NULL OR strpos(lower(coalesce(t.content,'') || ' ' || coalesce(t.file_name,'')), lower($9)) > 0) ORDER BY t.occurred_at_ms DESC,t.id DESC LIMIT $8",
        columns()
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(e)
        .bind(c)
        .bind(p)
        .bind(anchor)
        .bind(files)
        .bind(i64::from(limit) + 1)
        .bind(keyword)
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
        TimelineScope {
            workspace_id: s.workspace_id,
            instance_id: Some(s.instance_id),
            external_identity: Some(&s.external_identity),
        },
        c,
        p,
        l,
        false,
        None,
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
        TimelineScope {
            workspace_id: s.workspace_id,
            instance_id: Some(s.instance_id),
            external_identity: Some(&s.external_identity),
        },
        c,
        p,
        l,
        true,
        None,
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
    let sql = format!(
        "SELECT {} FROM communication_messages t {JOINS} WHERE t.scope_id=$1 AND t.instance_id=$2 AND ($3::text IS NULL OR t.external_identity=$3) AND t.id=$4 AND t.type='file'",
        columns()
    );
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
