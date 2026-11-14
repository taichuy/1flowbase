use crate::openapi_docs::{
    ApiDocsRegistry, DocsCatalog, DocsCatalogCategoryOperations, DocsCatalogOperation,
};
use control_plane::application_public_api::publications::ApplicationPublicationVersionRecord;
use domain::ApplicationRecord;
use serde_json::Value;
use std::sync::Arc;
#[derive(Debug, Clone)]
pub struct ApplicationPublicDocsContext {
    pub application: ApplicationRecord,
    pub active_publication: Option<ApplicationPublicationVersionRecord>,
    pub locale: String,
    pub assistant_operations: Vec<DocsCatalogOperation>,
    pub api_docs: Arc<ApiDocsRegistry>,
}

pub trait ApplicationPublicDocsPort: Send + Sync {
    fn catalog(&self, context: &ApplicationPublicDocsContext) -> DocsCatalog;
    fn category_operations(
        &self,
        context: &ApplicationPublicDocsContext,
        category_id: &str,
    ) -> Option<DocsCatalogCategoryOperations>;
    fn category_spec(
        &self,
        context: &ApplicationPublicDocsContext,
        category_id: &str,
    ) -> Option<Value>;
    fn operation_spec(
        &self,
        context: &ApplicationPublicDocsContext,
        operation_id: &str,
    ) -> Option<Value>;
}
