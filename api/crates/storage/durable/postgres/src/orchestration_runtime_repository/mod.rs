use anyhow::{anyhow, Result};
mod storage_maintenance;
use async_trait::async_trait;
use control_plane_contracts::{
    application_public_runtime::{
        ApplicationPublicConversationMessageRecord, ApplicationPublicConversationRecord,
        ApplicationPublicConversationRepository, ApplicationPublishedCallbackAttemptRepository,
        ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
        BindApplicationPublicConversationInput, CancelPublishedFlowRunInput,
        CreatePublishedFlowRunResult, ListApplicationPublicConversationMessagesInput,
        ListWaitingCallbackPublishedRunsInput, PublishedRunNodeUsage, PublishedRunPendingCallback,
        PublishedRunStreamState,
    },
    ports::{
        AckRuntimeEventDeliveryInput, AcquireResumeClaimInput, AcquireResumeClaimOutput,
        AppendBillingSessionInput, AppendCapabilityInvocationInput, AppendContextProjectionInput,
        AppendContextVersionInput, AppendCostLedgerInput, AppendCreditLedgerInput,
        AppendModelFailoverAttemptLedgerInput, AppendProviderInvocationContextInput,
        AppendRecoveryHistoryInput, AppendRunEventInput, AppendRuntimeEventInput,
        AppendRuntimeItemInput, AppendRuntimeSpanInput, AppendUsageLedgerInput,
        ApplicationRunCountTokensResult, ApplicationRunOverviewReadModel,
        ApplicationRunResumeTimelineReadModel, ApplicationRunResumeTimelineSummaryReadModel,
        ApplicationRunTraceChildrenCursor, ApplicationRunTraceProjectionStatistics,
        ApplicationRunTraceRefreshJob, AttachCompiledPlanToFlowRunInput, BillingRepository,
        BindInvocationContextInput, CallbackResumeAdmissionFence, CallbackResumeContext,
        CallbackResumeWaitingNode, ClaimRuntimeEventDeliveriesInput,
        ClearModelProviderRequestLogsBatchInput, ClearModelProviderRequestLogsBatchResult,
        CommitFlowRunTerminalInput, CommitFlowRunTerminalReceipt, CommitFlowRunTerminalResult,
        CommitToolCallbackResultsInput, CommitToolCallbackResultsOutput, CompleteCallbackTaskInput,
        CompleteFlowRunInput, CompleteNodeRunInput, ConvertLegacyRuntimeShadowBatchInput,
        ConvertLegacyRuntimeShadowBatchResult, CreateCallbackTaskInput, CreateCheckpointInput,
        CreateFlowRunInput, CreateFlowRunShellInput, CreateNodeRunInput,
        CreateRuntimeDebugArtifactInput, CreditReservation, CreditTransactionRecord,
        DataModelSideEffectReceiptClaim, DebugVariableCacheEntry,
        DeleteDebugVariableCacheEntriesInput, DeleteModelProviderRequestLogsInput,
        FailQueuedFlowRunShellInput, FinalizeModelBillingInput,
        FinalizePublishedRunMissingStreamTerminalPersistenceInput,
        FinalizePublishedRunMissingStreamTerminalPersistenceOutcome, FinalizedModelBilling,
        FinishFlowRunCallbackResumeAttemptInput, FinishResumeClaimInput,
        GetApplicationRunMonitoringReportInput, GetRuntimeDebugArtifactInput,
        LinkUsageLedgerToModelFailoverAttemptInput, ListApplicationConversationRunsPageInput,
        ListApplicationRunConversationMessageItemsPageInput, ListApplicationRunTraceChildrenPage,
        ListApplicationRunTraceChildrenPageInput, ListApplicationRunsPageInput,
        ListModelProviderRequestLogsPageInput, ListPricingRulesInput,
        MarkRuntimeEventDeliveryUncertainInput, ModelBillingAdmission, ModelBillingAdmissionInput,
        ModelProviderRequestLogsPage, OrchestrationRuntimeRepository, PersistWaitingKind,
        PersistWaitingStateInput, PersistedWaitingState, PutCanonicalRuntimeContentInput,
        RecordFlowRunCallbackResumeAttemptInput, RecordFlowRunCallbackResumeAttemptOutput,
        RecordModelObservationInput, RecordedModelObservation, ReleaseRuntimeEventDeliveryInput,
        RenewResumeClaimInput, ReplaceApplicationRunTraceProjectionInput, ReserveCreditInput,
        ResumeClaimDisposition, ResumeClaimKind, ResumeClaimRecord, ResumeClaimStatus,
        RollbackLegacyRuntimeShadowInput, RollbackLegacyRuntimeShadowResult,
        RuntimeContextContentVersion, RuntimeEventDeliveryClaim, RuntimeEventReplayWindow,
        SettleCreditInput, ToolCallbackRoundDisposition, UpdateCallbackTaskPayloadsInput,
        UpdateCheckpointPayloadsInput, UpdateFlowRunInput, UpdateFlowRunPayloadsInput,
        UpdateNodeRunInput, UpdateNodeRunPayloadsInput, UpdateRunEventPayloadInput,
        UpsertApplicationRunTraceProjectionStatusInput, UpsertCompiledPlanInput,
        UpsertDataModelSideEffectReceiptInput, UpsertDebugVariableCacheEntryInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};
pub use storage_maintenance::RuntimeStorageCompactionReceipt;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

mod agent_logs;
mod agent_logs_delete;
mod client_trajectory;
mod detail_queries;
mod json_storage;
mod log_query;
mod task_trace_sources;
mod trajectory;
use json_storage::{
    lossless_json_columns, lossless_json_parameter, lossless_json_value, lossless_text_parameter,
    original_optional_text, original_required_text,
};
mod record_mappers;
#[cfg(feature = "responses-history-migration")]
mod responses_history_migration;
mod sequencing;

use detail_queries::*;
use record_mappers::*;
use sequencing::*;

include!("event_methods.rs");
include!("delivery_methods.rs");
include!("artifact_methods.rs");
include!("application_run_log_methods.rs");
include!("application_run_logs/client_log_associations.rs");
include!("application_run_logs/assistant_conversation_methods.rs");
include!("application_run_logs/run_conversation_projection_methods.rs");
include!("application_run_logs/projection/native_canonical_projection.rs");
include!("application_run_logs/run_conversation_message_item_methods.rs");
#[cfg(test)]
#[path = "../_tests/application/request_history_context_tests.rs"]
mod request_history_context_tests;
include!("application_run_logs/task_projection_methods.rs");
include!("application_run_trace_projection_methods.rs");
include!("application_run_logs/trace_refresh_methods.rs");
include!("application_run_monitoring_methods.rs");
include!("application_run_logs/monitoring_usage.rs");
include!("debug_variable_cache_methods.rs");
include!("flow_run_methods.rs");
include!("flow_run_callback_resume_attempt_methods.rs");
include!("storage_foundation_methods.rs");
include!("legacy_shadow_methods.rs");
include!("waiting_state_methods.rs");
include!("tool_callback_inbox_methods.rs");
include!("resume_claim_methods.rs");
include!("ledger_methods.rs");
include!("read_methods.rs");
include!("request_log_methods.rs");
include!("side_effect_receipt_methods.rs");

#[path = "repository_ports/orchestration.rs"]
mod orchestration_port;
#[path = "repository_ports/published_flow_runs.rs"]
mod published_flow_runs_port;
#[path = "repository_ports/published_run_control.rs"]
mod published_run_control_port;
#[path = "repository_ports/callback_attempts.rs"]
mod callback_attempts_port;
#[path = "repository_ports/public_conversations.rs"]
mod public_conversations_port;
