//! Host-owned Privacy Masking kernel: built-in rules, regex engine, streaming restorer, and cache.
pub mod builtins;
pub mod engine;
pub mod kernel;
pub mod stream_restorer;

#[cfg(test)]
mod _tests;

pub use builtins::{
    builtin_rule_descriptors as list_builtin_rule_descriptors, BUILTIN_API_KEY_PREFIXED,
    BUILTIN_BANK_CARD, BUILTIN_CJK_NAMED_SECRET, BUILTIN_CN_ID_CARD, BUILTIN_CN_PHONE,
    BUILTIN_CONFIG_SECRET_ASSIGNMENT, BUILTIN_CRYPTO_PRIVATE_KEY, BUILTIN_EMAIL,
    BUILTIN_IPV4_PUBLIC, BUILTIN_JWT_TOKEN, BUILTIN_PRIVATE_KEY, BUILTIN_URL_PASSWORD,
};
pub use engine::{
    derive_hmac_subkey, deterministic_placeholder, placeholder_regex, prefix_hint,
    sanitize_placeholder_kind, CompiledWorkspacePolicy, PrivacyPolicyCompileError,
    StageMaskMatchAccumulator, StageMaskState,
};
pub use kernel::{
    ExecutedMaskingStage, PreparedPrivacyMasking, PreviewMaskingOutput, PrivacyInvocationContext,
    PrivacyMaskingKernel, PrivacyPolicySyncError, SyncPolicySummary,
};
pub use stream_restorer::{
    ProviderStreamEventRestorer, RestorationStack, RestorationStage, StreamingTextRestorer,
};
