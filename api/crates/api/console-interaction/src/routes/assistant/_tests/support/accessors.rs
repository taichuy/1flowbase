use super::*;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StateAssistantRuntimeToolInvokerFactory {
    pub fn test_new(state: std::sync::Weak<ApiState>) -> Self { Self { state } }
}
