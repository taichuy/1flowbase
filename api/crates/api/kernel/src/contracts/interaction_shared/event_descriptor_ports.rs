use crate::{error_response::ApiError,openapi_interface::{OpenApiCapabilityCatalogDependencies,OpenApiCapabilityCatalogEntry}};
#[async_trait::async_trait]
pub trait ApiEventDescriptorPort: Send + Sync {
    async fn workflow(&self, dependencies: &OpenApiCapabilityCatalogDependencies, actor: &domain::ActorContext, entry: &OpenApiCapabilityCatalogEntry) -> Result<Option<interface_runtime::DocumentOperationDescriptor>,ApiError>;
    async fn runtime_model(&self, dependencies: &OpenApiCapabilityCatalogDependencies, actor: &domain::ActorContext, entry: &OpenApiCapabilityCatalogEntry) -> Result<Option<interface_runtime::DocumentOperationDescriptor>,ApiError>;
}
