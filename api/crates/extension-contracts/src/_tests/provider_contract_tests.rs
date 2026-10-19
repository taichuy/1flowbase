use std::collections::{BTreeMap, BTreeSet};

use extension_contracts::provider_contract::{
    message_block_required_capabilities, recovery_receipt_from_details,
    semantic_required_capabilities, CommitLevel, CursorProvenance, ModelDiscoveryMode,
    NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
    NativePromptCacheControlType, ProtocolContextEnvelope, ProviderBalanceInfo,
    ProviderBalanceResult, ProviderCanonicalBlockKind, ProviderCompactError,
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensCoverage,
    ProviderCountTokensError, ProviderCountTokensFallbackReason, ProviderCountTokensInput,
    ProviderCountTokensResult, ProviderGenerateProjectionError,
    ProviderGenerateTranslationDecision, ProviderInvocationCapability, ProviderInvocationInput,
    ProviderInvocationResult, ProviderInvocationTerminationKind, ProviderInvocationTimingReceipt,
    ProviderInvocationTransportOutcome, ProviderLifecycleAckOutcome, ProviderLogicalSessionState,
    ProviderMessage, ProviderMessageRole, ProviderNativeTransport, ProviderNetworkEgressContext,
    ProviderNetworkEgressMode, ProviderOutputItemPhase, ProviderOutputProtocolFailure,
    ProviderPhysicalTransportState, ProviderProjectionErrorCode, ProviderProjectionFidelity,
    ProviderProjectionLossCode, ProviderProjectionSource, ProviderRecoveryDirective,
    ProviderRecoveryReceipt, ProviderResetCreditOperation, ProviderResetCreditResult,
    ProviderResetCreditRuntimeInput, ProviderRuntimeError, ProviderRuntimeErrorKind,
    ProviderRuntimeLine, ProviderStdioMethod, ProviderStdioRequest, ProviderStdioResponse,
    ProviderStreamEvent, ProviderToolCall, ProviderTransportClosureEvidence,
    ProviderTransportClosureOutcome, ProviderTransportClosureSource, ProviderTransportNoAckReason,
    ProviderTransportSessionAction, ProviderTransportSessionCloseReason,
    ProviderTransportSessionCommand, ProviderTransportSessionDirective,
    ProviderTransportSessionIdentity, ProviderTransportSessionReceipt, ProviderUsage,
    ProviderUsageWindow, ProviderUsageWindowsResult, ProviderWireOperation, RecoveryBudget,
    RecoveryDisposition, RecoveryPolicy, RecoveryReason, RecoveryTransport, SocketIncarnation,
    TransportEpoch, PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY,
    PROVIDER_INVOCATION_TIMING_SCHEMA_VERSION, PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY,
    PROVIDER_RECOVERY_RECEIPT_METADATA_KEY, PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY,
    PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY,
};
use serde_json::json;

mod invocation;
mod operations;
mod runtime_output;
mod transport_recovery;
