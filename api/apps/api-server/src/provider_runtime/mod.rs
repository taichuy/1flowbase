pub use api_host_kernel::provider_runtime::*;

pub(crate) fn services_for_current_cfg(services: ApiRuntimeServices) -> ApiRuntimeServices {
    #[cfg(test)]
    {
        services.with_local_artifact_references_for_test_support()
    }
    #[cfg(not(test))]
    {
        services
    }
}
#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
pub(crate) fn resolver_for_current_cfg(
    resolver: ModelProviderSlotResolver,
) -> ModelProviderSlotResolver {
    #[cfg(test)]
    {
        resolver.with_local_artifact_references_for_test_support()
    }
    #[cfg(not(test))]
    {
        resolver
    }
}
