use std::collections::HashMap;

use control_plane_contracts::ports::RunArchiveRepository;
use uuid::Uuid;

pub(crate) async fn load_node_run_error_payloads(
    store: &storage_durable_postgres::MainDurableStore,
    run_id: Uuid,
) -> Result<HashMap<String, serde_json::Value>, crate::error_response::ApiError> {
    let rows = <_ as RunArchiveRepository>::list_node_run_error_payloads(store, run_id).await?;
    Ok(rows
        .into_iter()
        .map(|(id, error_payload)| (id.to_string(), error_payload))
        .collect())
}
