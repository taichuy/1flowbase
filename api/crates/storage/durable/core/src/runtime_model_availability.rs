use anyhow::Result;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDataModelAvailability {
    Available,
    NotPublished,
    Disabled,
    Broken,
}

impl RuntimeDataModelAvailability {
    pub fn from_status(status: domain::DataModelStatus) -> Self {
        match status {
            domain::DataModelStatus::Published => Self::Available,
            domain::DataModelStatus::Draft => Self::NotPublished,
            domain::DataModelStatus::Disabled => Self::Disabled,
            domain::DataModelStatus::Broken => Self::Broken,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeModelError {
    #[error("runtime model unavailable: {0}")]
    Unavailable(String),
    #[error("runtime model not published: {0}")]
    NotPublished(String),
    #[error("runtime model disabled: {0}")]
    Disabled(String),
    #[error("runtime model broken: {0}")]
    Broken(String),
    #[error("runtime model record action not allowed: {model_code}:{action}")]
    RecordActionNotAllowed {
        model_code: String,
        action: &'static str,
    },
    #[error("runtime model create missing api required fields: {model_code}:{fields:?}")]
    MissingCreateRequiredFields {
        model_code: String,
        fields: Vec<String>,
    },
    #[error("runtime model operation invalid input: {0}")]
    InvalidOperationInput(&'static str),
    #[error("runtime model operation invalid field: {0}")]
    InvalidOperationField(String),
    #[error("runtime model ordered-tree adapter unavailable")]
    OrderedTreeUnavailable,
}

impl RuntimeModelError {
    pub fn unavailable(model_code: impl Into<String>) -> Self {
        Self::Unavailable(model_code.into())
    }

    pub fn not_published(model_code: impl Into<String>) -> Self {
        Self::NotPublished(model_code.into())
    }

    pub fn disabled(model_code: impl Into<String>) -> Self {
        Self::Disabled(model_code.into())
    }

    pub fn broken(model_code: impl Into<String>) -> Self {
        Self::Broken(model_code.into())
    }

    pub fn record_action_not_allowed(model_code: impl Into<String>, action: &'static str) -> Self {
        Self::RecordActionNotAllowed {
            model_code: model_code.into(),
            action,
        }
    }

    pub fn missing_create_required_fields<I, S>(model_code: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::MissingCreateRequiredFields {
            model_code: model_code.into(),
            fields: fields.into_iter().map(Into::into).collect(),
        }
    }
}

pub fn ensure_runtime_model_available(
    model_code: &str,
    availability: RuntimeDataModelAvailability,
) -> Result<()> {
    match availability {
        RuntimeDataModelAvailability::Available => Ok(()),
        RuntimeDataModelAvailability::NotPublished => {
            Err(RuntimeModelError::not_published(model_code).into())
        }
        RuntimeDataModelAvailability::Disabled => {
            Err(RuntimeModelError::disabled(model_code).into())
        }
        RuntimeDataModelAvailability::Broken => Err(RuntimeModelError::broken(model_code).into()),
    }
}
