//! Host-owned managed execution identities. These values describe a binding, not a grant.
//! Worker messages never deserialize an execution handle or supply trusted call identity.
use std::{num::NonZeroU64, sync::Arc};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use super::{ContributionId, DescriptorValueError};

macro_rules! managed_identity_segment {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DescriptorValueError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(DescriptorValueError::Empty {
                        kind: stringify!($name),
                    });
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
            }
        }
    };
}

managed_identity_segment!(ManagedInstallationId);
managed_identity_segment!(ManagedWorkspaceId);

macro_rules! managed_fingerprint {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn from_bytes(bytes: &[u8]) -> Self {
                Self(format!("sha256:{:x}", Sha256::digest(bytes)))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                if !value.strip_prefix("sha256:").is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                }) {
                    return Err(D::Error::custom(
                        "managed fingerprint must be a lowercase SHA-256 digest",
                    ));
                }
                Ok(Self(value))
            }
        }
    };
}

// Digest of the immutable installed manifest bytes. The artifact resolver owns materialization.
managed_fingerprint!(ManagedArtifactFingerprint);
// Digest of the canonical contribution descriptor and its exact execution binding.
managed_fingerprint!(ManagedBindingFingerprint);

/// Stable logical subject. Authorization revisions belong to this subject, not a worker mount.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContributionSubject {
    installation_id: ManagedInstallationId,
    workspace_id: ManagedWorkspaceId,
    contribution_id: ContributionId,
}

impl ManagedContributionSubject {
    pub fn new(
        installation_id: ManagedInstallationId,
        workspace_id: ManagedWorkspaceId,
        contribution_id: ContributionId,
    ) -> Self {
        Self {
            installation_id,
            workspace_id,
            contribution_id,
        }
    }
    pub fn installation_id(&self) -> &ManagedInstallationId {
        &self.installation_id
    }
    pub fn workspace_id(&self) -> &ManagedWorkspaceId {
        &self.workspace_id
    }
    pub fn contribution_id(&self) -> &ContributionId {
        &self.contribution_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedExecutionIdentity {
    subject: ManagedContributionSubject,
    artifact_fingerprint: ManagedArtifactFingerprint,
    binding_fingerprint: ManagedBindingFingerprint,
}

impl ManagedExecutionIdentity {
    pub fn new(
        installation_id: ManagedInstallationId,
        workspace_id: ManagedWorkspaceId,
        contribution_id: ContributionId,
        artifact_fingerprint: ManagedArtifactFingerprint,
        binding_fingerprint: ManagedBindingFingerprint,
    ) -> Self {
        Self {
            subject: ManagedContributionSubject::new(
                installation_id,
                workspace_id,
                contribution_id,
            ),
            artifact_fingerprint,
            binding_fingerprint,
        }
    }
    pub fn subject(&self) -> &ManagedContributionSubject {
        &self.subject
    }
    pub fn installation_id(&self) -> &ManagedInstallationId {
        self.subject.installation_id()
    }
    pub fn workspace_id(&self) -> &ManagedWorkspaceId {
        self.subject.workspace_id()
    }
    pub fn contribution_id(&self) -> &ContributionId {
        self.subject.contribution_id()
    }
    pub fn artifact_fingerprint(&self) -> &ManagedArtifactFingerprint {
        &self.artifact_fingerprint
    }
    pub fn binding_fingerprint(&self) -> &ManagedBindingFingerprint {
        &self.binding_fingerprint
    }
}

/// In-process object capability minted by the Runtime Host. Cloning retains the mount;
/// reconstructing the same public identity and generation cannot forge its private instance.
/// This handle is deliberately neither Serialize nor Deserialize. It carries no authorization.
#[derive(Debug, Clone)]
pub struct ManagedExecutionHandle {
    identity: ManagedExecutionIdentity,
    generation: NonZeroU64,
    instance: Arc<()>,
}

impl ManagedExecutionHandle {
    pub fn new(identity: ManagedExecutionIdentity, generation: NonZeroU64) -> Self {
        Self {
            identity,
            generation,
            instance: Arc::new(()),
        }
    }
    pub fn identity(&self) -> &ManagedExecutionIdentity {
        &self.identity
    }
    pub fn generation(&self) -> NonZeroU64 {
        self.generation
    }
}

impl PartialEq for ManagedExecutionHandle {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.generation == other.generation
            && Arc::ptr_eq(&self.instance, &other.instance)
    }
}
impl Eq for ManagedExecutionHandle {}
