use super::*;

pub(super) async fn conversations(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    e: Option<&str>,
    c: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationConversation>> {
    let rows=sqlx::query("SELECT * FROM communication_conversations WHERE workspace_id=$1 AND instance_id=$2 AND ($3::text IS NULL OR external_identity=$3) AND ($4::uuid IS NULL OR id<$4) ORDER BY id DESC LIMIT $5")
        .bind(w).bind(i).bind(e).bind(c).bind(i64::from(l.clamp(1,100))+1).fetch_all(store.pool()).await?;
    Ok(page(rows.into_iter().map(conversation).collect(), l, |v| {
        v.id
    }))
}
pub(super) async fn messages(
    store: &PgControlPlaneStore,
    s: &CommunicationHistoryScope,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationMessage>> {
    let sql=format!("SELECT m.*,({RETRY_ELIGIBILITY}) AS can_retry FROM communication_messages m JOIN communication_conversations c ON c.id=m.conversation_id WHERE c.workspace_id=$1 AND c.instance_id=$2 AND c.external_identity=$3 AND c.id=$4 AND ($5::uuid IS NULL OR m.id<$5) ORDER BY m.id DESC LIMIT $6");
    let rows = sqlx::query(&sql)
        .bind(s.workspace_id)
        .bind(s.instance_id)
        .bind(&s.external_identity)
        .bind(c)
        .bind(p)
        .bind(i64::from(l.clamp(1, 100)) + 1)
        .fetch_all(store.pool())
        .await?;
    Ok(page(
        rows.into_iter().map(message).collect::<Result<Vec<_>>>()?,
        l,
        |v| v.id,
    ))
}
pub(super) async fn files(
    store: &PgControlPlaneStore,
    s: &CommunicationHistoryScope,
    c: Uuid,
    p: Option<Uuid>,
    l: u32,
) -> Result<CommunicationPage<CommunicationAttachment>> {
    let rows=sqlx::query("SELECT a.* FROM communication_attachments a JOIN communication_messages m ON m.id=a.message_id JOIN communication_conversations c ON c.id=m.conversation_id WHERE c.workspace_id=$1 AND c.instance_id=$2 AND c.external_identity=$3 AND c.id=$4 AND ($5::uuid IS NULL OR a.id<$5) ORDER BY a.id DESC LIMIT $6")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(c).bind(p).bind(i64::from(l.clamp(1,100))+1).fetch_all(store.pool()).await?;
    Ok(page(
        rows.iter().map(attachment).collect::<Result<Vec<_>>>()?,
        l,
        |v| v.id,
    ))
}
