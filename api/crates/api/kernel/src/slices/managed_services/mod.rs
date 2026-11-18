pub mod registration;
pub use registration::ManagedServiceRegistration;
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ManagedServiceFailure(pub String);
impl ManagedServiceFailure {
    pub fn from_runtime(error: &anyhow::Error) -> Option<Self> {
        let runtime_core::runtime_backend::RuntimeBackendError::Contract(contract) =
            error.downcast_ref()?
        else {
            return None;
        };
        let extension_contracts::error::ExtensionContractError::InvalidProviderPackage { message } =
            contract.as_ref()
        else {
            return None;
        };
        if !message.is_empty()
            && message.len() <= 128
            && message
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            Some(Self(message.clone()))
        } else {
            None
        }
    }
}
