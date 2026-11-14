use super::*;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn compile_runtime_model_interface_registry_for_test() -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface::compile_registry_for_test()
}
