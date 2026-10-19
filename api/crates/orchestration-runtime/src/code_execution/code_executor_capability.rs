use std::{error::Error, fmt};

use crate::compiled_plan::{CodeExecutorCapability, CodeIsolationProfile, CompiledCodeDependency};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCodeExecutor {
    pub executor_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeExecutorSelectionErrorKind {
    ExecutorNotFound,
    UnsupportedLanguage,
    UnsupportedMode,
    UnsupportedArtifactTarget,
    LimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeExecutorSelectionError {
    pub kind: CodeExecutorSelectionErrorKind,
    pub field: &'static str,
    pub requested: String,
    pub executor_id: Option<String>,
    pub supported: Vec<String>,
}

impl CodeExecutorSelectionError {
    fn executor_not_found(profile: &CodeIsolationProfile) -> Self {
        Self {
            kind: CodeExecutorSelectionErrorKind::ExecutorNotFound,
            field: "executor_id",
            requested: profile.executor_id.clone(),
            executor_id: None,
            supported: Vec::new(),
        }
    }

    fn unsupported_language(capability: &CodeExecutorCapability, language: &str) -> Self {
        Self {
            kind: CodeExecutorSelectionErrorKind::UnsupportedLanguage,
            field: "language",
            requested: language.to_string(),
            executor_id: Some(capability.executor_id.clone()),
            supported: capability.supported_languages.clone(),
        }
    }

    fn unsupported_mode(capability: &CodeExecutorCapability, mode: &str) -> Self {
        Self {
            kind: CodeExecutorSelectionErrorKind::UnsupportedMode,
            field: "mode",
            requested: mode.to_string(),
            executor_id: Some(capability.executor_id.clone()),
            supported: capability.supported_modes.clone(),
        }
    }

    fn unsupported_artifact_target(capability: &CodeExecutorCapability, target: &str) -> Self {
        Self {
            kind: CodeExecutorSelectionErrorKind::UnsupportedArtifactTarget,
            field: "dependency.target",
            requested: target.to_string(),
            executor_id: Some(capability.executor_id.clone()),
            supported: capability.supported_artifact_targets.clone(),
        }
    }

    fn limit_exceeded(
        capability: &CodeExecutorCapability,
        field: &'static str,
        requested: impl ToString,
        supported: impl ToString,
    ) -> Self {
        Self {
            kind: CodeExecutorSelectionErrorKind::LimitExceeded,
            field,
            requested: requested.to_string(),
            executor_id: Some(capability.executor_id.clone()),
            supported: vec![supported.to_string()],
        }
    }

    pub fn stable_code(&self) -> &'static str {
        match self.kind {
            CodeExecutorSelectionErrorKind::ExecutorNotFound => "executor_not_found",
            CodeExecutorSelectionErrorKind::UnsupportedLanguage => "unsupported_language",
            CodeExecutorSelectionErrorKind::UnsupportedMode => "unsupported_mode",
            CodeExecutorSelectionErrorKind::UnsupportedArtifactTarget => {
                "unsupported_artifact_target"
            }
            CodeExecutorSelectionErrorKind::LimitExceeded => "limit_exceeded",
        }
    }
}

impl fmt::Display for CodeExecutorSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.executor_id.as_deref() {
            Some(executor_id) => write!(
                formatter,
                "{}: executor `{}` does not support {} `{}`",
                self.stable_code(),
                executor_id,
                self.field,
                self.requested
            ),
            None => write!(
                formatter,
                "{}: code executor `{}` is not available",
                self.stable_code(),
                self.requested
            ),
        }
    }
}

impl Error for CodeExecutorSelectionError {}

pub fn select_code_executor(
    profile: &CodeIsolationProfile,
    language: &str,
    dependencies: &[CompiledCodeDependency],
    capabilities: &[CodeExecutorCapability],
) -> Result<SelectedCodeExecutor, CodeExecutorSelectionError> {
    let capability = capabilities
        .iter()
        .find(|capability| capability.executor_id == profile.executor_id)
        .ok_or_else(|| CodeExecutorSelectionError::executor_not_found(profile))?;

    if !contains_case_insensitive(&capability.supported_languages, language) {
        return Err(CodeExecutorSelectionError::unsupported_language(
            capability, language,
        ));
    }
    if !capability
        .supported_modes
        .iter()
        .any(|mode| mode == &profile.mode)
    {
        return Err(CodeExecutorSelectionError::unsupported_mode(
            capability,
            &profile.mode,
        ));
    }
    if profile.timeout_ms > capability.max_timeout_ms {
        return Err(CodeExecutorSelectionError::limit_exceeded(
            capability,
            "timeout_ms",
            profile.timeout_ms,
            capability.max_timeout_ms,
        ));
    }
    if profile.memory_mb > capability.max_memory_mb {
        return Err(CodeExecutorSelectionError::limit_exceeded(
            capability,
            "memory_mb",
            profile.memory_mb,
            capability.max_memory_mb,
        ));
    }
    if profile.stack_kb > capability.max_stack_kb {
        return Err(CodeExecutorSelectionError::limit_exceeded(
            capability,
            "stack_kb",
            profile.stack_kb,
            capability.max_stack_kb,
        ));
    }

    for dependency in dependencies {
        if !capability
            .supported_artifact_targets
            .iter()
            .any(|target| target == &dependency.target)
        {
            return Err(CodeExecutorSelectionError::unsupported_artifact_target(
                capability,
                &dependency.target,
            ));
        }
    }

    Ok(SelectedCodeExecutor {
        executor_id: capability.executor_id.clone(),
    })
}

fn contains_case_insensitive(values: &[String], requested: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(requested))
}
