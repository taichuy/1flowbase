use std::{fmt, sync::Arc};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityError {
    #[error("{kind} must not be empty")]
    Empty { kind: &'static str },
    #[error("{kind} contains an unsupported character")]
    UnsupportedCharacter { kind: &'static str },
    #[error("route method must contain only ASCII alphabetic characters")]
    InvalidMethod,
    #[error("route path must be an absolute API path")]
    InvalidRoutePath,
}

macro_rules! identity {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(value: impl AsRef<str>) -> Result<Self, IdentityError> {
                let value = value.as_ref().trim();
                validate_identity(value, $kind)?;
                Ok(Self(Arc::from(value)))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_ref()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

identity!(InterfaceId, "interface identity");
identity!(InterfaceVersion, "interface version");
identity!(BindingId, "binding identity");
identity!(AuthorizationOperation, "authorization operation");
identity!(HandlerReference, "handler reference");
identity!(TargetReference, "target reference");
identity!(InterfaceOwner, "interface owner");
identity!(GraphFingerprint, "graph fingerprint");
identity!(RegistryFingerprint, "registry fingerprint");
identity!(BindingFingerprint, "binding fingerprint");
identity!(PlanFingerprint, "plan fingerprint");
identity!(
    AuthenticationActivationIdentity,
    "authentication activation identity"
);
identity!(
    AuthorizationDecisionFingerprint,
    "authorization decision fingerprint"
);
identity!(
    AuthenticationAdapterReference,
    "authentication adapter reference"
);
identity!(
    AuthorizationAdapterReference,
    "authorization adapter reference"
);
identity!(AdmissionAdapterReference, "admission adapter reference");
identity!(ExtensionPlanFingerprint, "extension plan fingerprint");
identity!(PluginIdentity, "plugin identity");
identity!(ArtifactIdentity, "artifact identity");
identity!(RuntimeTargetIdentity, "runtime target identity");
identity!(RuntimeGeneration, "runtime generation");
identity!(WorkerGeneration, "worker generation");

fn validate_identity(value: &str, kind: &'static str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty { kind });
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/' | b'@')
    }) {
        return Err(IdentityError::UnsupportedCharacter { kind });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractIdentity {
    contract_id: Arc<str>,
    version: Arc<str>,
}

impl ContractIdentity {
    pub fn new(
        contract_id: impl AsRef<str>,
        version: impl AsRef<str>,
    ) -> Result<Self, IdentityError> {
        let contract_id = contract_id.as_ref().trim();
        let version = version.as_ref().trim();
        validate_identity(contract_id, "contract identity")?;
        validate_identity(version, "contract version")?;
        Ok(Self {
            contract_id: Arc::from(contract_id),
            version: Arc::from(version),
        })
    }

    pub fn contract_id(&self) -> &str {
        self.contract_id.as_ref()
    }

    pub fn version(&self) -> &str {
        self.version.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteIdentity {
    method: Arc<str>,
    path: Arc<str>,
}

impl RouteIdentity {
    pub fn new(method: impl AsRef<str>, path: impl AsRef<str>) -> Result<Self, IdentityError> {
        let method = method.as_ref().trim();
        if method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(IdentityError::InvalidMethod);
        }
        let path = path.as_ref().trim();
        if !path.starts_with("/api/") || path.contains('?') || path.contains('#') {
            return Err(IdentityError::InvalidRoutePath);
        }
        Ok(Self {
            method: Arc::from(method.to_ascii_uppercase()),
            path: Arc::from(path),
        })
    }

    pub fn method(&self) -> &str {
        self.method.as_ref()
    }

    pub fn path(&self) -> &str {
        self.path.as_ref()
    }
}
