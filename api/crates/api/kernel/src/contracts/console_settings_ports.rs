//! State-free documentation aggregation contract; the actual OpenAPI merge stays at the composition root.
#[async_trait::async_trait]
pub trait WorkflowExtensionDocumentProvider: Send + Sync + 'static {
    async fn document(
        &self,
        store: storage_durable_postgres::MainDurableStore,
        cookie_name: &str,
        template_catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    ) -> Result<serde_json::Value, crate::error_response::ApiError>;
}
