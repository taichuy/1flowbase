use super::*;
pub fn compile_registry_for_test(
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    compile_registry(Arc::new(UnavailablePublicSignInPort))
}

struct UnavailablePublicSignInPort;

impl PublicSignInPort for UnavailablePublicSignInPort {
    fn sign_in(&self, _input: PublicSignInInput) -> PublicSignInFuture<'_> {
        Box::pin(async {
            Err(PublicSignInTargetError(
                anyhow::anyhow!("test public sign-in port is unavailable").into(),
            ))
        })
    }
}

#[doc(hidden)]
impl PublicSignInInput {
    pub fn test_new(value: LoginCommand) -> Self {
        Self(value)
    }
}
#[doc(hidden)]
impl PublicSignInOutput {
    pub fn test_new(value: LoginResult) -> Self {
        Self(value)
    }
}
#[doc(hidden)]
impl PublicSignInTargetError {
    pub fn test_new(value: ApiError) -> Self {
        Self(value)
    }
}

#[doc(hidden)]
impl PublicSignInInput {
    pub fn test_inner(&self) -> &LoginCommand {
        &self.0
    }
}
#[doc(hidden)]
impl PublicSignInOutput {
    pub fn test_inner(&self) -> &LoginResult {
        &self.0
    }
}
#[doc(hidden)]
impl PublicSignInTargetError {
    pub fn test_inner(&self) -> &ApiError {
        &self.0
    }
}
