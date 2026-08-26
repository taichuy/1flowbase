use std::collections::HashMap;

use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn load_node_run_error_payloads(
    store: &storage_durable_postgres::MainDurableStore,
    run_id: Uuid,
) -> Result<HashMap<String, serde_json::Value>, crate::error_response::ApiError> {
    let rows = sqlx::query(
        r#"
        select id, error_payload
        from node_runs
        where flow_run_id = $1
          and error_payload is not null
        "#,
    )
    .bind(run_id)
    .fetch_all(store.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<Uuid, _>("id").to_string(),
                row.get::<serde_json::Value, _>("error_payload"),
            )
        })
        .collect())
}
