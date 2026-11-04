//! Opt-in fixtures; no production constructor switches behavior with this feature.
#[path = "../_tests/application/application_runtime_routes.rs"]
pub mod application;
mod resources;
#[path = "../_tests/support/mod.rs"]
pub mod support;
pub(crate) use resources::TestResources;

pub async fn refresh_next_trace_projection(
    state: &crate::app_state::ApiState,
) -> anyhow::Result<bool> {
    crate::workers::trace_projection::refresh_next_trace_projection(state).await
}

pub async fn rebuild_trace_projection(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: uuid::Uuid,
    flow_run_id: uuid::Uuid,
) -> anyhow::Result<()> {
    crate::workers::trace_projection::rebuild_trace_projection(store, application_id, flow_run_id)
        .await
}
