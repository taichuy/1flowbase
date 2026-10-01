//! Transition-local immutable I/O; a mutable node record is never a replay source.
use super::*;

pub(super) async fn archive(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &AppendRuntimeEventInput,
) -> Result<(Value, Option<Uuid>, Option<Uuid>)> {
    let Some(fields) = input.payload.as_object() else {
        return Ok((input.payload.clone(), None, None));
    };
    if fields.keys().any(|key| key.contains('\0')) || fields.contains_key("_node_payload_ref") {
        return Ok((input.payload.clone(), None, None));
    }
    let names = [
        "input_payload",
        "output_payload",
        "error_payload",
        "metrics_payload",
    ];
    let body: serde_json::Map<String, Value> = names
        .iter()
        .filter_map(|name| {
            fields
                .get(*name)
                .map(|value| ((*name).into(), Value::String(value.to_string())))
        })
        .collect();
    if body.is_empty() {
        return Ok((input.payload.clone(), None, None));
    }
    // JSON text operators in PostgreSQL cannot inspect NUL-bearing metadata.
    // Keep this exceptional shape inline; archived field values use source strings.
    let mut metadata = fields.clone();
    for name in names {
        metadata.remove(name);
    }
    if lossless_json_value(&Value::Object(metadata))[1].is_string() {
        return Ok((input.payload.clone(), None, None));
    }
    let (scope, application) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "select scope_id,application_id from flow_runs where id=$1",
    )
    .bind(input.flow_run_id)
    .fetch_one(&mut **tx)
    .await?;
    let (content, _, _, created) =
        put_canonical_runtime_content_with_creation(tx, scope, application, &Value::Object(body))
            .await?;
    if created {
        sqlx::query("insert into runtime_observation_body_ownership(content_id) values($1)")
            .bind(content)
            .execute(&mut **tx)
            .await?;
    }
    let mut payload = fields.clone();
    for name in names {
        payload.remove(name);
    }
    payload.insert(
        "_node_payload_ref".into(),
        json!({
            "version":1,"content_id":content,"application_id":application
        }),
    );
    Ok((Value::Object(payload), Some(content), None))
}
