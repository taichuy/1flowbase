//! Shared read-only frontend catalog inputs; catalog business stays with its owner.
use std::sync::Arc;
#[derive(Clone)]
pub struct FrontendBlockDependencies {
    pub store: storage_durable_postgres::MainDurableStore,
    pub api_node_id: String,
    pub graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
}
