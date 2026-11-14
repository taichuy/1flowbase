//! Composition-only Settings adapters and transitional plugin route contribution.

pub(crate) struct WorkflowExtensionDocument;
#[async_trait::async_trait]
impl api_host_kernel::console_settings_ports::WorkflowExtensionDocumentProvider
    for WorkflowExtensionDocument
{
    async fn document(
        &self,
        store: storage_durable_postgres::MainDurableStore,
        cookie_name: &str,
        template_catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    ) -> Result<serde_json::Value, crate::error_response::ApiError> {
        crate::openapi::workflow_extension_openapi_document_with(
            store,
            cookie_name,
            template_catalog,
        )
        .await
    }
}
