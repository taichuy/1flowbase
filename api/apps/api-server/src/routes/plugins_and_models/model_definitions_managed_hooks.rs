//! The boot-compiled Create bridge consumes one invocation-owned managed graph, never latest.
use extension_contracts::{
    HookPhase, ManagedCreateHookInput, ManagedCreateView, ManagedHookInvocation,
    ManagedHookOutcome, ManagedHookTerminal,
};
use interface_runtime::*;
use plugin_framework::extension_bus::{ContributionId, ContributionResolutionStatus};
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use std::{
    any::Any,
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{ModelDefinitionsInput, ModelDefinitionsOutput};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    extension_bus::{ManagedExtensionComposition, ManagedWorkspaceSnapshot},
};

const CREATE: &str = "model_definitions.create";
const PHASES: [HookPhase; 6] = [
    HookPhase::Authorization,
    HookPhase::Admission,
    HookPhase::Before,
    HookPhase::After,
    HookPhase::Failure,
    HookPhase::Completion,
];

struct FrozenWorkspaceHooks {
    composition: Arc<ManagedExtensionComposition>,
    snapshot: Arc<ManagedWorkspaceSnapshot>,
    _invocation_reference: Box<dyn Send + Sync>,
    create: ManagedCreateView,
    stages: BTreeMap<HookPhase, Vec<ContributionId>>,
}

/// This private typed value cannot be deserialized, replaced per phase or authored by a worker.
struct FrozenManagedCreateInvocation {
    invocation_id: InvocationId,
    registry_fingerprint: RegistryFingerprint,
    principal: RuntimeExecutionPrincipal,
    workspace: Option<FrozenWorkspaceHooks>,
}

pub(crate) async fn freeze<I: InterfaceContract>(
    state: &ApiState,
    registry: &Arc<CompiledInterfaceRegistry>,
    envelope: InvocationEnvelope<I, UserPrincipal>,
) -> Result<InvocationEnvelope<I, UserPrincipal>, ApiError> {
    let Some(plan) = registry.plan(envelope.binding_id()) else {
        return Ok(envelope);
    };
    if plan.definition().interface_id().as_str() != CREATE {
        return Ok(envelope);
    }
    let Some(ModelDefinitionsInput::Create(body)) =
        (envelope.input() as &dyn Any).downcast_ref::<ModelDefinitionsInput>()
    else {
        return Err(anyhow::anyhow!("Create binding requires its typed Create input").into());
    };
    for phase in PHASES {
        if !plan.extension_plan().registrations().iter().any(|entry| {
            entry.registration().plugin() == &bridge_identity(phase)
                && entry.registration().point() == interface_point(phase)
        }) {
            return Err(
                anyhow::anyhow!("Create managed bridge is absent from the compiled plan").into(),
            );
        }
    }
    let actor = envelope.principal().actor();
    let deadline = envelope
        .controls()
        .deadline()
        .unwrap_or_else(|| SystemTime::now() + Duration::from_secs(30));
    let mut frozen = FrozenManagedCreateInvocation {
        invocation_id: envelope.lineage().invocation_id(),
        registry_fingerprint: registry.fingerprint().clone(),
        principal: RuntimeExecutionPrincipal {
            workspace_id: actor.current_workspace_id.to_string(),
            actor_id: Some(actor.user_id.to_string()),
            deadline_unix_ms: i64::try_from(
                deadline
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| anyhow::anyhow!("Create deadline expired"))?
                    .as_millis(),
            )?,
        },
        workspace: None,
    };
    // System and invalid scope strings retain the existing domain admission/validation behavior.
    // A workspace installation is never asked to inspect those targets.
    if body.scope_kind == "workspace" {
        if let Ok(composition) = state.provider_runtime.managed_composition() {
            if let Some(snapshot) = composition.snapshot(actor.current_workspace_id).await {
                let invocation_reference = snapshot.freeze_reference()?;
                let boot = state
                    .extension_boot_snapshot
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Create boot graph missing"))?;
                // Compare the host baseline provenance and exact point contracts before combining views.
                for provenance in boot.graph().module_provenance() {
                    if !snapshot.graph.module_provenance().contains(provenance) {
                        return Err(anyhow::anyhow!(
                            "managed Create graph has a different host baseline"
                        )
                        .into());
                    }
                }
                let mut stages = BTreeMap::new();
                for phase in PHASES {
                    let point_id = match phase {
                        HookPhase::Authorization => {
                            "1flowbase.model-definitions.create.authorization"
                        }
                        HookPhase::Admission => "1flowbase.model-definitions.create.admission",
                        HookPhase::Before => "1flowbase.model-definitions.create.before",
                        HookPhase::After => "1flowbase.model-definitions.create.after",
                        HookPhase::Failure => "1flowbase.model-definitions.create.failure",
                        HookPhase::Completion => "1flowbase.model-definitions.create.completion",
                    };
                    let point = snapshot
                        .graph
                        .points()
                        .iter()
                        .find(|point| point.descriptor().point_id.as_str() == point_id)
                        .ok_or_else(|| anyhow::anyhow!("managed Create point is missing"))?;
                    let host_point = boot
                        .graph()
                        .points()
                        .iter()
                        .find(|point| point.descriptor().point_id.as_str() == point_id)
                        .ok_or_else(|| anyhow::anyhow!("host Create point is missing"))?;
                    if point.descriptor() != host_point.descriptor()
                        || point.provenance() != host_point.provenance()
                    {
                        return Err(anyhow::anyhow!(
                            "managed Create point contract differs from the host baseline"
                        )
                        .into());
                    }
                    let mut ids = Vec::new();
                    for contribution in point.contributions() {
                        let id = &contribution.descriptor().contribution_id;
                        if !snapshot.authority.contributions.contains_key(id) {
                            continue;
                        }
                        let binding = snapshot.bindings.get(id).ok_or_else(|| {
                            anyhow::anyhow!("managed Create executable binding missing")
                        })?;
                        if binding.handle.identity().workspace_id().as_str()
                            != actor.current_workspace_id.to_string()
                            || &binding.descriptor != contribution.descriptor()
                            || !snapshot
                                .graph
                                .contribution_receipts()
                                .iter()
                                .any(|receipt| {
                                    &receipt.descriptor().contribution_id == id
                                        && receipt.status() == &ContributionResolutionStatus::Active
                                })
                        {
                            return Err(anyhow::anyhow!(
                                "managed Create binding does not match the effective plan"
                            )
                            .into());
                        }
                        ids.push(id.clone());
                    }
                    stages.insert(phase, ids);
                }
                frozen.workspace = Some(FrozenWorkspaceHooks {
                    _invocation_reference: Box::new(invocation_reference),
                    composition: composition.clone(),
                    snapshot,
                    create: ManagedCreateView {
                        code: body.code.clone(),
                        template_provider: body.template_provider.clone(),
                        template_code: body.template_code.clone(),
                        template_version: body.template_version.clone(),
                    },
                    stages,
                });
            }
        }
    }
    envelope
        .freeze_extension_context(Arc::new(frozen))
        .map_err(|_| anyhow::anyhow!("Create invocation was already frozen").into())
}

fn phase_input(phase: HookPhase, create: &ManagedCreateView) -> ManagedCreateHookInput {
    match phase {
        HookPhase::Authorization => ManagedCreateHookInput::Authorization {
            create: create.clone(),
        },
        HookPhase::Admission => ManagedCreateHookInput::Admission {
            create: create.clone(),
        },
        HookPhase::Before => ManagedCreateHookInput::Before {
            create: create.clone(),
        },
        HookPhase::After => ManagedCreateHookInput::After {
            model_id: String::new(),
        },
        HookPhase::Failure => ManagedCreateHookInput::Failure {
            classification: String::new(),
        },
        HookPhase::Completion => ManagedCreateHookInput::Completion {
            terminal: ManagedHookTerminal::Succeeded,
        },
    }
}
fn interface_point(phase: HookPhase) -> InterfaceExtensionPoint {
    match phase {
        HookPhase::Authorization => InterfaceExtensionPoint::Authorization,
        HookPhase::Admission => InterfaceExtensionPoint::Admission,
        HookPhase::Before => InterfaceExtensionPoint::Before,
        HookPhase::After => InterfaceExtensionPoint::After,
        HookPhase::Failure => InterfaceExtensionPoint::Failure,
        HookPhase::Completion => InterfaceExtensionPoint::Completion,
    }
}
fn bridge_identity(phase: HookPhase) -> PluginIdentity {
    PluginIdentity::new(format!("api-server.managed-create.{phase:?}").to_lowercase())
        .expect("static bridge identity")
}

struct ManagedCreateBridge;
impl ManagedCreateBridge {
    async fn run(
        &self,
        context: &InterfaceHookContext,
        phase: HookPhase,
        terminal_input: Option<ManagedCreateHookInput>,
    ) -> Result<(), &'static str> {
        let frozen = context
            .extension_context::<FrozenManagedCreateInvocation>()
            .ok_or("managed-create-context-missing")?;
        if frozen.invocation_id != context.invocation_id()
            || &frozen.registry_fingerprint != context.registry_fingerprint()
            || context
                .principal()
                .and_then(|p| p.workspace_id())
                .map(|id| id.to_string())
                .as_deref()
                != Some(frozen.principal.workspace_id.as_str())
            || context
                .principal()
                .and_then(|p| p.user_id())
                .map(|id| id.to_string())
                != frozen.principal.actor_id
        {
            return Err("managed-create-context-mismatch");
        }
        let Some(workspace) = &frozen.workspace else {
            return Ok(());
        };
        let input = terminal_input.unwrap_or_else(|| phase_input(phase, &workspace.create));
        let ids = workspace
            .stages
            .get(&phase)
            .ok_or("managed-create-phase-missing")?;
        for id in ids {
            let mut principal = frozen.principal.clone();
            if input.is_observer() {
                // Completion has its own Kernel-owned budget, independent of business cancellation.
                principal.deadline_unix_ms =
                    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
                        + 1_000;
            }
            let result = workspace
                .composition
                .execute_hook(
                    &workspace.snapshot,
                    id,
                    principal,
                    ManagedHookInvocation {
                        invocation_id: frozen.invocation_id.value().to_string(),
                        registry_fingerprint: frozen.registry_fingerprint.as_str().into(),
                        graph_fingerprint: workspace.snapshot.graph.fingerprint().as_str().into(),
                        authority_revision: 0,
                    },
                    input.clone(),
                )
                .await;
            match result {
                Ok(ManagedHookOutcome::Continue | ManagedHookOutcome::Observed) => {}
                Ok(
                    outcome @ (ManagedHookOutcome::Deny { .. } | ManagedHookOutcome::Failed { .. }),
                ) => {
                    tracing::warn!(invocation_id = %frozen.invocation_id.value(), contribution_id = id.as_str(), ?phase, "managed Create worker rejected or failed");
                    if input.is_observer() {
                        context.report_observer_failure();
                    } else {
                        return Err(if matches!(outcome, ManagedHookOutcome::Deny { .. }) {
                            "managed-create-rejected"
                        } else {
                            "managed-create-execution-failed"
                        });
                    }
                }
                Err(error) => {
                    tracing::warn!(invocation_id = %frozen.invocation_id.value(), contribution_id = id.as_str(), ?phase, %error, "managed Create worker execution failed");
                    if input.is_observer() {
                        context.report_observer_failure();
                    } else {
                        return Err("managed-create-execution-failed");
                    }
                }
            }
        }
        Ok(())
    }
}
impl InterfaceAuthorizationContribution for ManagedCreateBridge {
    fn authorize(
        &self,
        request: InterfaceAuthorizationContributionRequest,
    ) -> InterfaceAuthorizationContributionFuture<'_> {
        Box::pin(async move {
            self.run(request.context(), HookPhase::Authorization, None)
                .await
                .map_err(InterfaceAuthorizationContributionError::classified)
        })
    }
}
impl InterfaceAdmissionContribution for ManagedCreateBridge {
    fn admit(
        &self,
        request: InterfaceAdmissionContributionRequest,
    ) -> InterfaceAdmissionContributionFuture<'_> {
        Box::pin(async move {
            self.run(request.context(), HookPhase::Admission, None)
                .await
                .map_err(InterfaceAdmissionContributionError::classified)
        })
    }
}
impl InterfaceBeforeHook<ModelDefinitionsInput> for ManagedCreateBridge {
    fn before<'a>(
        &'a self,
        context: InterfaceHookContext,
        input: &'a mut ModelDefinitionsInput,
    ) -> InterfaceBeforeHookFuture<'a> {
        Box::pin(async move {
            if !matches!(input, ModelDefinitionsInput::Create(_)) {
                return Err(InterfaceBeforeHookError::classified(
                    "managed-create-input-mismatch",
                ));
            }
            self.run(&context, HookPhase::Before, None)
                .await
                .map_err(InterfaceBeforeHookError::classified)
        })
    }
}
impl InterfaceAfterHook<ModelDefinitionsOutput> for ManagedCreateBridge {
    fn after<'a>(
        &'a self,
        context: InterfaceHookContext,
        output: &'a ModelDefinitionsOutput,
    ) -> InterfaceAfterHookFuture<'a> {
        Box::pin(async move {
            if let ModelDefinitionsOutput::Model(model) = output {
                if self
                    .run(
                        &context,
                        HookPhase::After,
                        Some(ManagedCreateHookInput::After {
                            model_id: model.id.clone(),
                        }),
                    )
                    .await
                    .is_err()
                {
                    context.report_observer_failure();
                }
            } else {
                context.report_observer_failure();
            }
        })
    }
}
impl InterfaceFailureHook for ManagedCreateBridge {
    fn failed<'a>(
        &'a self,
        context: InterfaceHookContext,
        classification: &'a str,
    ) -> InterfaceFailureHookFuture<'a> {
        Box::pin(async move {
            if self
                .run(
                    &context,
                    HookPhase::Failure,
                    Some(ManagedCreateHookInput::Failure {
                        classification: classification.into(),
                    }),
                )
                .await
                .is_err()
            {
                context.report_observer_failure();
            }
        })
    }
}
impl InterfaceCompletionHook for ManagedCreateBridge {
    fn completed(
        &self,
        context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            let terminal = match terminal {
                InterfaceInvocationTerminal::Completed => ManagedHookTerminal::Succeeded,
                InterfaceInvocationTerminal::Cancelled => ManagedHookTerminal::Cancelled,
                InterfaceInvocationTerminal::Rejected => ManagedHookTerminal::Rejected,
                InterfaceInvocationTerminal::Failed => ManagedHookTerminal::Failed,
            };
            if self
                .run(
                    &context,
                    HookPhase::Completion,
                    Some(ManagedCreateHookInput::Completion { terminal }),
                )
                .await
                .is_err()
            {
                context.report_observer_failure();
            }
        })
    }
}

pub(super) fn bind(
    compiler: &mut RegistryCompiler,
    graph: GraphFingerprint,
) -> Result<(), RegistryCompilationError> {
    let id = InterfaceId::new(CREATE).expect("static Create identity");
    let bridge = Arc::new(ManagedCreateBridge);
    for phase in PHASES {
        let (permission, facts) = match phase {
            HookPhase::Authorization => (
                InterfaceExtensionPermission::Authorize,
                vec![
                    InterfaceExtensionFact::PrincipalSummary,
                    InterfaceExtensionFact::DefinitionIdentity,
                ],
            ),
            HookPhase::Admission => (
                InterfaceExtensionPermission::Admit,
                vec![
                    InterfaceExtensionFact::PrincipalSummary,
                    InterfaceExtensionFact::AuthorizationDecision,
                ],
            ),
            HookPhase::Before => (
                InterfaceExtensionPermission::ObserveInput,
                vec![InterfaceExtensionFact::TypedInput],
            ),
            HookPhase::After => (
                InterfaceExtensionPermission::ObserveOutput,
                vec![InterfaceExtensionFact::TypedOutput],
            ),
            HookPhase::Failure => (
                InterfaceExtensionPermission::ObserveFailure,
                vec![InterfaceExtensionFact::FailureClassification],
            ),
            HookPhase::Completion => (
                InterfaceExtensionPermission::ObserveCompletion,
                vec![InterfaceExtensionFact::Terminal],
            ),
        };
        compiler.register_extension(
            &id,
            100,
            InterfaceExtensionRegistration::new(
                bridge_identity(phase),
                InterfaceExtensionTier::HostExtension,
                interface_point(phase),
                permission,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                facts,
            )?,
        )?;
    }
    compiler.bind_authorization_plan(
        &id,
        Arc::new(
            TypedInterfaceAuthorizationPlan::<ModelDefinitionsInput, ModelDefinitionsOutput>::new(
                graph.clone(),
            )
            .bind(bridge_identity(HookPhase::Authorization), bridge.clone()),
        ),
    )?;
    compiler.bind_admission_plan(
        &id,
        Arc::new(
            TypedInterfaceAdmissionPlan::<ModelDefinitionsInput, ModelDefinitionsOutput>::new(
                graph.clone(),
            )
            .bind(bridge_identity(HookPhase::Admission), bridge.clone()),
        ),
    )?;
    compiler.bind_hook_plan(
        &id,
        Arc::new(
            TypedInterfaceHookPlan::<ModelDefinitionsInput, ModelDefinitionsOutput>::new(graph)
                .bind_before(bridge_identity(HookPhase::Before), bridge.clone())
                .bind_after(bridge_identity(HookPhase::After), bridge.clone())
                .bind_failure(bridge_identity(HookPhase::Failure), bridge.clone())
                .bind_completion(bridge_identity(HookPhase::Completion), bridge),
        ),
    )?;
    Ok(())
}
