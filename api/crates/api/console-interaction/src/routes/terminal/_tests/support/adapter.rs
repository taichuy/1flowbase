use super::*;
#[doc(hidden)]
impl TerminalAdapter {
    pub fn test_from_weak(state: Weak<ApiState>) -> Self {
        Self::from_weak(state)
    }
    pub fn test_host(&self) -> Weak<dyn api_host_kernel::terminal_host::TerminalHostOperations> {
        self.host.clone()
    }
    pub async fn test_run(
        &self,
        principal: &interface_runtime::UserPrincipal,
        input: TerminalInput,
    ) -> Result<TerminalOutput, ApiError> {
        self.run(principal, input).await
    }
}
