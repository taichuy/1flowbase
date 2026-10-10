//! Boot-frozen managed services. Package declarations become ordinary host-owned console APIs.
mod interface;
mod registration;
mod routes;
pub(crate) use interface::registry_contribution;
pub(crate) use registration::{append_openapi, load, ManagedServiceRegistration};
pub(crate) use routes::route_assembly;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct ManagedServiceFailure(pub(crate) String);
impl ManagedServiceFailure {
    fn from_runtime(error: &anyhow::Error) -> Option<Self> {
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

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
