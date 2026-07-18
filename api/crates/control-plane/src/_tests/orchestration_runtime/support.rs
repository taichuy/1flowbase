use super::*;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::capability_plugin_runtime::{
    CapabilityExecutionOutput, ResolveCapabilityOptionsInput, ResolveCapabilityOutputSchemaInput,
    ValidateCapabilityConfigInput,
};
use crate::ports::{
    AppendBillingSessionInput, AppendCapabilityInvocationInput, AppendContextProjectionInput,
    AppendCostLedgerInput, AppendCreditLedgerInput, AppendModelFailoverAttemptLedgerInput,
    AppendRunEventInput, AppendRuntimeEventInput, AppendRuntimeItemInput, AppendRuntimeSpanInput,
    AppendUsageLedgerInput, CallbackResumeContext, CallbackResumeWaitingNode, CompleteFlowRunInput,
    CompleteNodeRunInput, CreateCallbackTaskInput, CreateCheckpointInput, CreateFlowRunInput,
    CreateModelDefinitionInput, CreateNodeRunInput, CreateScopeDataModelGrantInput,
    DataModelSideEffectReceiptClaim, DebugVariableCacheEntry, DeleteDebugVariableCacheEntriesInput,
    LinkUsageLedgerToModelFailoverAttemptInput, ModelDefinitionRepository,
    OrchestrationRuntimeRepository, UpdateFlowRunInput, UpdateModelDefinitionInput,
    UpdateModelFieldInput, UpdateNodeRunInput, UpdateScopeDataModelGrantInput,
    UpsertCompiledPlanInput, UpsertDataModelSideEffectReceiptInput,
    UpsertDebugVariableCacheEntryInput,
};
use plugin_framework::provider_contract::{ProviderInvocationResult, ProviderStreamEvent};

use crate::{
    flow::InMemoryFlowRepository,
    ports::{
        ApplicationEnvironmentVariableInput, ApplicationJsDependencySelectionRepository,
        ApplicationVisibility, CreateApplicationInput, CreateApplicationTagInput,
        DeleteApplicationInput, ReplaceApplicationEnvironmentVariablesInput,
        ReplaceApplicationJsDependencySelectionInput, UpdateApplicationInput,
    },
};

#[path = "support/fixtures/mod.rs"]
mod fixtures;
#[path = "support/repository/mod.rs"]
mod repository;

pub(crate) use repository::{InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime};
