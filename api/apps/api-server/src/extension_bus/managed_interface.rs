//! Canonical host bridge: one frozen workspace graph for all phases and actual stream terminal.
use super::{ManagedExtensionComposition, ManagedWorkspaceSnapshot};
use crate::app_state::ApiState;
use extension_contracts::{
    HookPhase, ManagedHookInvocation, ManagedHookOutcome, ManagedHookTerminal,
    ManagedInterfaceInput, ManagedInterfaceView, ManagedProjectionContract,
};
use interface_runtime::*;
use plugin_framework::extension_bus::{
    ContributionId, ContributionResolutionStatus, EffectiveExtensionGraph, MANAGED_INTERFACE_PHASES,
};
use runtime_core::runtime_backend::{RuntimeExecutionPrincipal, RuntimeManagedHookInput};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type Contracts = BTreeMap<ContractIdentity, ManagedProjectionContract>;

pub(crate) struct ManagedInterfaceFactory {
    state: Weak<ApiState>,
    baseline: Arc<EffectiveExtensionGraph>,
    contracts: OnceLock<Arc<Contracts>>,
}
impl ManagedInterfaceFactory {
    pub(crate) fn new(state: &Arc<ApiState>, baseline: Arc<EffectiveExtensionGraph>) -> Arc<Self> {
        Arc::new(Self {
            state: Arc::downgrade(state),
            baseline,
            contracts: OnceLock::new(),
        })
    }
    pub(crate) fn bind_registry(&self, registry: &CompiledInterfaceRegistry) -> anyhow::Result<()> {
        let mut contracts = BTreeMap::new();
        let mut schema_failures = Vec::new();
        for entry in registry.managed_contracts() {
            let contract = ManagedProjectionContract {
                contract_id: entry.contract.contract_id().into(),
                contract_version: entry.contract.version().into(),
                schema: entry
                    .schema
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("compiled managed schema missing"))?,
            };
            if let Err(error) = contract.compile() {
                schema_failures.push(format!(
                    "contract_id={} version={} rust_type={} schema_bytes={}: {}",
                    contract.contract_id,
                    contract.contract_version,
                    entry.rust_type,
                    serde_json::to_vec(&contract.schema)?.len(),
                    error,
                ));
            }
            contracts.insert(entry.contract.clone(), contract);
        }
        anyhow::ensure!(
            schema_failures.is_empty(),
            "managed schema compilation rejected {} contract(s):\n{}",
            schema_failures.len(),
            schema_failures.join("\n")
        );
        for definition in registry.definitions() {
            for identity in [
                Some(definition.input_contract()),
                Some(definition.output_contract()),
                Some(definition.target_error_contract()),
                definition.stream_event_contract(),
            ]
            .into_iter()
            .flatten()
            {
                anyhow::ensure!(
                    contracts.contains_key(identity),
                    "compiled interface contract has no safe schema"
                );
            }
        }
        anyhow::ensure!(
            registry.bindings().all(|binding| registry
                .plan(binding.binding_id())
                .is_some_and(CompiledInvocationPlan::has_managed_invocation_bridge)),
            "compiled binding lacks managed bridge"
        );
        self.contracts
            .set(Arc::new(contracts))
            .map_err(|_| anyhow::anyhow!("managed schemas already frozen"))
    }
}

struct WorkspaceInvocation {
    composition: Arc<ManagedExtensionComposition>,
    snapshot: Arc<ManagedWorkspaceSnapshot>,
    _reference: Box<dyn Send + Sync>,
    stages: BTreeMap<HookPhase, Vec<ContributionId>>,
}
struct FrozenInvocation {
    invocation: InvocationId,
    registry: RegistryFingerprint,
    interface_id: String,
    interface_version: String,
    input: Option<ManagedInterfaceProjection>,
    contracts: Arc<Contracts>,
    principal: Option<RuntimeExecutionPrincipal>,
    workspace: Option<WorkspaceInvocation>,
}

impl ManagedInterfaceInvocationFactory for ManagedInterfaceFactory {
    fn freeze(&self, request: ManagedInterfaceFreezeRequest) -> ManagedInterfaceFreezeFuture<'_> {
        Box::pin(async move {
            let contracts = self
                .contracts
                .get()
                .cloned()
                .ok_or("managed-catalog-not-frozen")?;
            let mut frozen = FrozenInvocation {
                invocation: request.context.invocation_id(),
                registry: request.context.registry_fingerprint().clone(),
                interface_id: request.definition.interface_id().as_str().into(),
                interface_version: request.definition.version().as_str().into(),
                input: request.input,
                contracts,
                principal: None,
                workspace: None,
            };
            // Execution workspace comes only from the sealed principal, never from business target fields.
            let Some(principal) = request.context.principal() else {
                return Ok(Arc::new(frozen) as Arc<dyn ManagedInterfaceInvocation>);
            };
            let Some(workspace_id) = principal.workspace_id() else {
                return Ok(Arc::new(frozen) as Arc<dyn ManagedInterfaceInvocation>);
            };
            let state = self.state.upgrade().ok_or("managed-host-unavailable")?;
            let Ok(composition) = state.provider_runtime.managed_composition() else {
                return Ok(Arc::new(frozen) as Arc<dyn ManagedInterfaceInvocation>);
            };
            let Some(snapshot) = composition.snapshot(workspace_id).await else {
                return Ok(Arc::new(frozen) as Arc<dyn ManagedInterfaceInvocation>);
            };
            let reference = snapshot
                .freeze_reference()
                .map_err(|_| "managed-snapshot-retired")?;
            for provenance in self.baseline.module_provenance() {
                if !snapshot.graph.module_provenance().contains(provenance) {
                    return Err("managed-host-baseline-mismatch");
                }
            }
            let module = composition
                .interface_module()
                .ok_or("managed-point-catalog-missing")?;
            let mut stages = BTreeMap::new();
            for phase in MANAGED_INTERFACE_PHASES {
                let id = extension_contracts::managed_interface_hook_point_id(
                    &frozen.interface_id,
                    phase,
                );
                let declared = module
                    .extension_points
                    .iter()
                    .find(|point| point.point_id.as_str() == id)
                    .ok_or("managed-point-not-declared")?;
                let point = snapshot
                    .graph
                    .points()
                    .iter()
                    .find(|point| point.descriptor().point_id.as_str() == id)
                    .ok_or("managed-point-not-compiled")?;
                if point.descriptor() != declared {
                    return Err("managed-point-contract-mismatch");
                }
                let mut ids = Vec::new();
                for contribution in point.contributions() {
                    let id = &contribution.descriptor().contribution_id;
                    if !snapshot.authority.contributions.contains_key(id) {
                        continue;
                    }
                    let binding = snapshot
                        .bindings
                        .get(id)
                        .ok_or("managed-executable-missing")?;
                    if binding.handle.identity().workspace_id().as_str() != workspace_id.to_string()
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
                        return Err("managed-frozen-binding-mismatch");
                    }
                    ids.push(id.clone());
                }
                stages.insert(phase, ids);
            }
            let deadline = request
                .deadline
                .unwrap_or_else(|| SystemTime::now() + Duration::from_secs(30));
            let deadline_unix_ms = i64::try_from(
                deadline
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "managed-deadline-expired")?
                    .as_millis(),
            )
            .map_err(|_| "managed-deadline-invalid")?;
            frozen.principal = Some(RuntimeExecutionPrincipal {
                workspace_id: workspace_id.to_string(),
                actor_id: principal.user_id().map(|id| id.to_string()),
                deadline_unix_ms,
            });
            frozen.workspace = Some(WorkspaceInvocation {
                composition: composition.clone(),
                snapshot,
                _reference: Box::new(reference),
                stages,
            });
            Ok(Arc::new(frozen) as Arc<dyn ManagedInterfaceInvocation>)
        })
    }
}

impl FrozenInvocation {
    fn view(
        &self,
        projection: Option<&ManagedInterfaceProjection>,
    ) -> Result<ManagedInterfaceView, &'static str> {
        let projection = projection.ok_or("managed-projection-limit")?;
        let contract = self
            .contracts
            .get(projection.contract())
            .ok_or("managed-contract-not-frozen")?;
        if &contract.schema != projection.schema() {
            return Err("managed-schema-changed");
        }
        let view = ManagedInterfaceView {
            contract: contract.clone(),
            value: projection.value().clone(),
        };
        view.validate().map_err(|_| "managed-projection-invalid")?;
        Ok(view)
    }
}
impl ManagedInterfaceInvocation for FrozenInvocation {
    fn run(
        &self,
        context: InterfaceHookContext,
        call: ManagedInterfaceCall,
    ) -> ManagedInterfaceCallFuture<'_> {
        Box::pin(async move {
            if context.invocation_id() != self.invocation
                || context.registry_fingerprint() != &self.registry
            {
                return Err("managed-invocation-mismatch");
            }
            let Some(workspace) = &self.workspace else {
                return Ok(());
            };
            let principal = self.principal.as_ref().ok_or("managed-principal-missing")?;
            if context
                .principal()
                .and_then(|p| p.workspace_id())
                .map(|id| id.to_string())
                .as_deref()
                != Some(principal.workspace_id.as_str())
                || context
                    .principal()
                    .and_then(|p| p.user_id())
                    .map(|id| id.to_string())
                    != principal.actor_id
            {
                return Err("managed-principal-mismatch");
            }
            let phase = match &call {
                ManagedInterfaceCall::Authorization => HookPhase::Authorization,
                ManagedInterfaceCall::Admission => HookPhase::Admission,
                ManagedInterfaceCall::Before => HookPhase::Before,
                ManagedInterfaceCall::After(_) => HookPhase::After,
                ManagedInterfaceCall::Failure => HookPhase::Failure,
                ManagedInterfaceCall::Completion(_) => HookPhase::Completion,
            };
            let ids = workspace
                .stages
                .get(&phase)
                .ok_or("managed-phase-missing")?;
            if ids.is_empty() {
                return Ok(());
            }
            let input = match call {
                ManagedInterfaceCall::Authorization => ManagedInterfaceInput::Authorization {
                    input: self.view(self.input.as_ref())?,
                },
                ManagedInterfaceCall::Admission => ManagedInterfaceInput::Admission {
                    input: self.view(self.input.as_ref())?,
                },
                ManagedInterfaceCall::Before => ManagedInterfaceInput::Before {
                    input: self.view(self.input.as_ref())?,
                },
                ManagedInterfaceCall::After(output) => ManagedInterfaceInput::After {
                    output: self.view(output.as_ref())?,
                },
                ManagedInterfaceCall::Failure => ManagedInterfaceInput::Failure {
                    classification: "host-target-failed".into(),
                },
                ManagedInterfaceCall::Completion(terminal) => ManagedInterfaceInput::Completion {
                    terminal: match terminal {
                        InterfaceInvocationTerminal::Completed => ManagedHookTerminal::Succeeded,
                        InterfaceInvocationTerminal::Cancelled => ManagedHookTerminal::Cancelled,
                        InterfaceInvocationTerminal::Rejected => ManagedHookTerminal::Rejected,
                        InterfaceInvocationTerminal::Failed => ManagedHookTerminal::Failed,
                    },
                },
            };
            let observer = matches!(
                phase,
                HookPhase::After | HookPhase::Failure | HookPhase::Completion
            );
            for id in ids {
                let mut principal = principal.clone();
                if observer {
                    principal.deadline_unix_ms =
                        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
                            + 1000;
                }
                let outcome = workspace
                    .composition
                    .execute_hook(
                        &workspace.snapshot,
                        id,
                        principal,
                        ManagedHookInvocation {
                            invocation_id: self.invocation.value().to_string(),
                            registry_fingerprint: self.registry.as_str().into(),
                            graph_fingerprint: workspace
                                .snapshot
                                .graph
                                .fingerprint()
                                .as_str()
                                .into(),
                            authority_revision: 0,
                        },
                        RuntimeManagedHookInput::Interface {
                            interface_id: self.interface_id.clone(),
                            interface_version: self.interface_version.clone(),
                            input: input.clone(),
                        },
                    )
                    .await;
                match outcome {
                    Ok(ManagedHookOutcome::Continue | ManagedHookOutcome::Observed) => {}
                    Ok(ManagedHookOutcome::Deny { .. }) if !observer => {
                        return Err("managed-interface-rejected");
                    }
                    _ if observer => context.report_observer_failure(),
                    _ => return Err("managed-interface-execution-failed"),
                }
            }
            Ok(())
        })
    }
}
