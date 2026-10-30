pub(super) struct RejectAttachExecutor;

impl crate::routes::application_public_api::native_read_interface::NativeRuntimeInvokerFactory
    for RejectAttachExecutor
{
    fn without_mcp(
        &self,
    ) -> Result<
        std::sync::Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        crate::routes::application_public_api::native::NativeApiError,
    > {
        panic!("an attached callback must not create an executor")
    }

    fn for_actor<'a>(
        &'a self,
        _actor: &'a control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> crate::routes::application_public_api::native_read_interface::RuntimeInvokerFuture<'a>
    {
        Box::pin(async { panic!("an attached callback must not create an executor") })
    }
}
