use super::*;

pub(super) async fn build_provider_runtime_config<R, H>(
    repository: &R,
    runtime: &H,
    master_key: &str,
    package: &ProviderPackage,
    installation: &domain::LocalPluginInstallationRecord,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value>
where
    R: ModelProviderRepository + PluginRepository,
    H: ProviderRuntimePort,
{
    crate::model_provider::instances::maintain_provider_runtime_config(
        repository,
        runtime,
        master_key,
        package,
        installation,
        instance,
    )
    .await
}
