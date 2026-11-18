use super::*;
/// Test assembly uses the production schema owner, without a duplicate ownership registry.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub async fn apply_fixture_managed_schema(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: Uuid,
    manifest: &plugin_framework::PluginManifestV1,
) -> Result<(), ApiError> {
    let declaration = ManagedSchemaDeclaration::from_manifest(manifest)?;
    if let Some(prepared) =
        prepare_managed_schema(dependencies, workspace_id, declaration.as_ref()).await?
    {
        prepared.apply(dependencies).await?;
    }
    Ok(())
}
