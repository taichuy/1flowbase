use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::provider_contract::ProviderRuntimeError;

pub type FrameworkResult<T> = Result<T, PluginFrameworkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFrameworkErrorKind {
    InvalidAssignment,
    InvalidProviderPackage,
    InvalidProviderContract,
    RuntimeContract,
    Io,
    Serialization,
}

#[derive(Debug, Error)]
pub enum PluginFrameworkError {
    #[error("invalid assignment: {message}")]
    InvalidAssignment { message: String },
    #[error("invalid provider package: {message}")]
    InvalidProviderPackage { message: String },
    #[error("invalid provider contract: {message}")]
    InvalidProviderContract { message: String },
    #[error("plugin runtime target {package_target} is incompatible with host {host_target}")]
    PackageRuntimeTargetMismatch {
        package_target: String,
        host_target: String,
    },
    #[error("provider runtime error: {error}")]
    RuntimeContract { error: Box<ProviderRuntimeError> },
    #[error("I/O error while loading provider package{path_display}: {message}")]
    Io {
        path: Option<PathBuf>,
        message: String,
        path_display: String,
    },
    #[error("serialization error while loading provider package{path_display}: {message}")]
    Serialization {
        path: Option<PathBuf>,
        message: String,
        path_display: String,
    },
}

impl PluginFrameworkError {
    pub fn kind(&self) -> PluginFrameworkErrorKind {
        match self {
            Self::InvalidAssignment { .. } => PluginFrameworkErrorKind::InvalidAssignment,
            Self::InvalidProviderPackage { .. } => PluginFrameworkErrorKind::InvalidProviderPackage,
            Self::InvalidProviderContract { .. } => {
                PluginFrameworkErrorKind::InvalidProviderContract
            }
            Self::PackageRuntimeTargetMismatch { .. } => {
                PluginFrameworkErrorKind::InvalidProviderContract
            }
            Self::RuntimeContract { .. } => PluginFrameworkErrorKind::RuntimeContract,
            Self::Io { .. } => PluginFrameworkErrorKind::Io,
            Self::Serialization { .. } => PluginFrameworkErrorKind::Serialization,
        }
    }

    pub fn invalid_assignment(message: impl Into<String>) -> Self {
        Self::InvalidAssignment {
            message: message.into(),
        }
    }

    pub fn invalid_provider_package(message: impl Into<String>) -> Self {
        Self::InvalidProviderPackage {
            message: message.into(),
        }
    }

    pub fn invalid_provider_contract(message: impl Into<String>) -> Self {
        Self::InvalidProviderContract {
            message: message.into(),
        }
    }

    pub fn package_runtime_target_mismatch(
        package_target: impl Into<String>,
        host_target: impl Into<String>,
    ) -> Self {
        Self::PackageRuntimeTargetMismatch {
            package_target: package_target.into(),
            host_target: host_target.into(),
        }
    }

    pub fn runtime(error: ProviderRuntimeError) -> Self {
        Self::RuntimeContract {
            error: Box::new(error),
        }
    }

    pub fn io(path: Option<&Path>, message: impl Into<String>) -> Self {
        let owned_path = path.map(Path::to_path_buf);
        let path_display = owned_path
            .as_ref()
            .map(|value| format!(" at {}", value.display()))
            .unwrap_or_default();
        Self::Io {
            path: owned_path,
            message: message.into(),
            path_display,
        }
    }

    pub fn serialization(path: Option<&Path>, message: impl Into<String>) -> Self {
        let owned_path = path.map(Path::to_path_buf);
        let path_display = owned_path
            .as_ref()
            .map(|value| format!(" at {}", value.display()))
            .unwrap_or_default();
        Self::Serialization {
            path: owned_path,
            message: message.into(),
            path_display,
        }
    }
}
