//! Store observation bodies as immutable, scoped content while retaining the
//! producer's step identity and exact original body representation.
use super::*;

pub(super) async fn archive_payload(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &AppendRuntimeEventInput,
) -> Result<(Value, Option<Uuid>)> {
    if !matches!(
        input.event_type.as_str(),
        "provider_semantic_step" | "provider_protocol_observation"
    ) || input.payload.get("body_ref").is_some()
    {
        return Ok((input.payload.clone(), None));
    }
    // PostgreSQL JSON can preserve NUL keys, but json_each cannot expose them as
    // text. Retain the exact inline representation for this exceptional shape.
    if input
        .payload
        .as_object()
        .is_some_and(|fields| fields.keys().any(|key| key.contains('\0')))
    {
        return Ok((input.payload.clone(), None));
    }
    let Some(body) = input.payload.get("body") else {
        return Ok((input.payload.clone(), None));
    };
    let (scope_id, application_id) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "select scope_id,application_id from flow_runs where id=$1",
    )
    .bind(input.flow_run_id)
    .fetch_one(&mut **tx)
    .await?;
    let (content_id, _, _, created) =
        put_canonical_runtime_content_with_creation(tx, scope_id, application_id, body).await?;
    if created {
        sqlx::query("insert into runtime_observation_body_ownership(content_id) values($1)")
            .bind(content_id)
            .execute(&mut **tx)
            .await?;
    }
    let mut payload = input.payload.clone();
    let object = payload
        .as_object_mut()
        .ok_or_else(|| anyhow!("observation payload must be an object"))?;
    object.remove("body");
    object.insert(
        "_observation_body_ref".into(),
        json!({"content_id":content_id,"application_id":application_id}),
    );
    Ok((payload, Some(content_id)))
}
