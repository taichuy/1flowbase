use crate::ConsoleLogEntry;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    SourceMissing,
    ModuleImportDenied,
    DynamicImportDenied,
    DependencyAliasInvalid,
    DependencyArtifactInvalid,
    SyntaxError,
    MainMissing,
    InvalidOutput,
    RuntimeError,
    Timeout,
    Cancelled,
    OutputLimit,
    InvalidLimits,
    ArtifactDescriptorInvalid,
    ArtifactVersionMismatch,
    ArtifactPathEscape,
    ArtifactMissing,
    ArtifactInvalid,
    ArtifactIntegrityMismatch,
    ArtifactSizeMismatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionError {
    pub(crate) kind: ErrorKind,
    pub console_logs: Vec<ConsoleLogEntry>,
}
impl From<ErrorKind> for ExecutionError {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind,
            console_logs: Vec::new(),
        }
    }
}
impl ExecutionError {
    pub fn code(&self) -> &'static str {
        use ErrorKind::*;
        match self.kind {
            SourceMissing => "source_missing",
            ModuleImportDenied | DynamicImportDenied => "module_import_denied",
            DependencyAliasInvalid => "dependency_alias_invalid",
            DependencyArtifactInvalid => "dependency_artifact_invalid",
            SyntaxError => "syntax_error",
            MainMissing => "main_missing",
            InvalidOutput => "invalid_output",
            RuntimeError => "runtime_error",
            Timeout => "timeout",
            Cancelled => "cancelled",
            OutputLimit => "output_limit_exceeded",
            InvalidLimits => "invalid_limits",
            ArtifactDescriptorInvalid => "artifact_descriptor_invalid",
            ArtifactVersionMismatch => "artifact_version_mismatch",
            ArtifactPathEscape => "artifact_path_escape",
            ArtifactMissing => "artifact_missing",
            ArtifactInvalid => "artifact_invalid",
            ArtifactIntegrityMismatch => "artifact_integrity_mismatch",
            ArtifactSizeMismatch => "artifact_size_mismatch",
        }
    }
    fn message(&self) -> &'static str {
        use ErrorKind::*;
        match self.kind {
            SourceMissing => "code source is required",
            ModuleImportDenied => "import and export syntax is not supported",
            DynamicImportDenied => "dynamic import is not supported",
            DependencyAliasInvalid => "dependency alias is invalid",
            DependencyArtifactInvalid => "dependency artifact is invalid",
            SyntaxError => "code source is not valid JavaScript",
            MainMissing => "main function is required",
            InvalidOutput => "main must return a JSON object",
            RuntimeError => "code execution failed",
            Timeout => "code execution timed out",
            Cancelled => "code execution cancelled",
            OutputLimit => "code output exceeds byte limit",
            InvalidLimits => "execution limits are invalid",
            ArtifactDescriptorInvalid => "packaged script descriptor is invalid",
            ArtifactVersionMismatch => "packaged script version mismatch",
            ArtifactPathEscape => "packaged script escapes version root",
            ArtifactMissing => "packaged script is missing",
            ArtifactInvalid => "packaged script must be bounded regular UTF-8 text",
            ArtifactIntegrityMismatch => "packaged script integrity mismatch",
            ArtifactSizeMismatch => "packaged script byte length mismatch",
        }
    }
}
impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}
impl std::error::Error for ExecutionError {}
