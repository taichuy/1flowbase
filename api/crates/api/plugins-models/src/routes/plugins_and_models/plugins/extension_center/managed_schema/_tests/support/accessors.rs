use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl ManagedSchemaDeclaration {
    pub fn test_new(owner: PluginSchemaOwner, contributions: Vec<PluginDataModelContribution>) -> Self { Self { owner, contributions } }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl PreparedManagedSchema {
    pub fn test_preview(&self) -> &ManagedSchemaPreviewResponse { &self.preview }
    pub fn test_plan(&self) -> &ManagedSchemaPlan { &self.plan }
}
