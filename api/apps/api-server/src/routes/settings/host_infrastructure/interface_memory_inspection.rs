use std::{future::Future, pin::Pin, sync::Arc};

use access_control::{ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup};
use control_plane::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{AuthRepository, RoleConsolePolicyReader},
};
use interface_runtime::InterfaceContract;
use storage_durable_postgres::MainDurableStore;

use super::{
    memory_support::{
        empty_memory_entry_page, empty_memory_tree_page, format_memory_reveal_mode,
        format_memory_value_state, memory_contract_definitions, memory_contract_label,
        memory_contract_stats_response, memory_contract_summary, memory_contract_supported,
        memory_inspection_target, memory_page_request, memory_query_path, parse_memory_reveal_mode,
        MemoryInspectionDependencies,
    },
    to_memory_entry_metadata_response, to_memory_tree_node_response, MemoryEntriesResponse,
    MemoryEntryRevealBody, MemoryEntryValueResponse, MemoryOverviewResponse, MemoryPageQuery,
    MemoryPathQuery, MemorySearchQuery, MemoryStatsOverviewResponse, MemoryStatsResponse,
    MemoryTreeResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum MemoryInspectionInput {
    Overview,
    StatsOverview,
    Entries {
        contract_code: String,
        query: MemoryPageQuery,
    },
    Stats {
        contract_code: String,
        query: MemoryPathQuery,
    },
    Tree {
        contract_code: String,
        query: MemoryPageQuery,
    },
    Search {
        contract_code: String,
        query: MemorySearchQuery,
    },
    Reveal {
        contract_code: String,
        body: MemoryEntryRevealBody,
    },
}

impl InterfaceContract for MemoryInspectionInput {
    const CONTRACT_ID: &'static str = "console-host-infrastructure-memory-inspection-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed inspection output is projected immediately into the console response"
)]
pub(crate) enum MemoryInspectionOutput {
    Overview(MemoryOverviewResponse),
    StatsOverview(MemoryStatsOverviewResponse),
    Entries(MemoryEntriesResponse),
    Stats(MemoryStatsResponse),
    Tree(MemoryTreeResponse),
    Revealed(MemoryEntryValueResponse),
}

impl InterfaceContract for MemoryInspectionOutput {
    const CONTRACT_ID: &'static str = "console-host-infrastructure-memory-inspection-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct MemoryInspectionInterfaceDependencies {
    pub(crate) memory: MemoryInspectionDependencies,
    pub(crate) audit_policy: Arc<dyn MemoryAuditPolicyPort>,
}

pub(crate) type MemoryAuditPolicyFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send + 'a>>;

pub(crate) trait MemoryAuditPolicyPort: Send + Sync {
    fn can_manage<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> MemoryAuditPolicyFuture<'a, bool>;

    fn append_reveal_audit<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        payload: serde_json::Value,
    ) -> MemoryAuditPolicyFuture<'a, ()>;
}

pub(crate) struct MemoryAuditPolicyAdapter {
    store: MainDurableStore,
    console_registry: Arc<ConsoleOperationRegistry>,
}

impl MemoryAuditPolicyAdapter {
    pub(crate) fn new(
        store: MainDurableStore,
        console_registry: Arc<ConsoleOperationRegistry>,
    ) -> Self {
        Self {
            store,
            console_registry,
        }
    }
}

impl MemoryAuditPolicyPort for MemoryAuditPolicyAdapter {
    fn can_manage<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> MemoryAuditPolicyFuture<'a, bool> {
        Box::pin(async move {
            if actor.is_root {
                return Ok(true);
            }
            let policies = self
                .store
                .load_role_console_policies_for_user(actor)
                .await?;
            Ok(has_registered_simple_operations(
                &self.console_registry,
                actor,
                &policies,
                &["host_infrastructure.memory.reveal"],
            ))
        })
    }

    fn append_reveal_audit<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        payload: serde_json::Value,
    ) -> MemoryAuditPolicyFuture<'a, ()> {
        Box::pin(async move {
            let workspace_id = (actor.current_workspace_id != domain::SYSTEM_SCOPE_ID)
                .then_some(actor.current_workspace_id);
            AuthRepository::append_audit_log(
                &self.store,
                &audit_log(
                    workspace_id,
                    Some(actor.user_id),
                    "host_infrastructure_memory",
                    None,
                    "host_infrastructure.memory_value_revealed",
                    payload,
                ),
            )
            .await?;
            Ok(())
        })
    }
}

struct MemoryInspectionAdapter(MemoryInspectionInterfaceDependencies);

impl MemoryInspectionAdapter {
    async fn execute_inner(
        &self,
        principal: &interface_runtime::UserPrincipal,
        input: MemoryInspectionInput,
    ) -> Result<MemoryInspectionOutput, ApiError> {
        let actor = principal.actor();
        let memory = &self.0.memory;
        match input {
            MemoryInspectionInput::Overview => {
                let mut contracts = Vec::new();
                for (contract_code, label) in memory_contract_definitions() {
                    contracts.push(memory_contract_summary(memory, contract_code, label).await?);
                }
                Ok(MemoryInspectionOutput::Overview(MemoryOverviewResponse {
                    can_manage: self.0.audit_policy.can_manage(actor).await?,
                    contracts,
                }))
            }
            MemoryInspectionInput::StatsOverview => {
                let inspection_path = Vec::new();
                let mut contracts = Vec::new();
                let mut total = control_plane::ports::EphemeralInspectionSummarySnapshot::empty();
                for (contract_code, label) in memory_contract_definitions() {
                    let stats = memory_contract_stats_response(
                        memory,
                        contract_code,
                        label,
                        &inspection_path,
                    )
                    .await?;
                    total.entry_count += stats.entry_count;
                    total.sensitive_entry_count += stats.sensitive_entry_count;
                    total.total_value_size_bytes += stats.total_value_size_bytes;
                    contracts.push(stats);
                }
                Ok(MemoryInspectionOutput::StatsOverview(
                    MemoryStatsOverviewResponse {
                        inspection_path,
                        contracts,
                        entry_count: total.entry_count,
                        sensitive_entry_count: total.sensitive_entry_count,
                        total_value_size_bytes: total.total_value_size_bytes,
                    },
                ))
            }
            MemoryInspectionInput::Entries {
                contract_code,
                query,
            } => {
                let label = memory_contract_label(&contract_code)?;
                let target = memory_inspection_target(memory, &contract_code)?;
                let capabilities = target.capabilities();
                let supported = memory_contract_supported(&capabilities);
                let page_request =
                    memory_page_request(query.path, query.cursor, query.limit, query.byte_limit);
                let page = if capabilities.list_entries {
                    target.list_entry_page(page_request).await?
                } else {
                    empty_memory_entry_page(page_request)
                };
                Ok(MemoryInspectionOutput::Entries(MemoryEntriesResponse {
                    contract_code: contract_code.clone(),
                    label: label.to_string(),
                    provider_code: memory.provider_codes.get(&contract_code).cloned(),
                    capabilities: capabilities.into(),
                    supported,
                    inspection_path: page.inspection_path,
                    entries: page
                        .entries
                        .into_iter()
                        .map(to_memory_entry_metadata_response)
                        .collect(),
                    next_cursor: page.next_cursor,
                    limit: page.limit,
                    byte_limit: page.byte_limit,
                    emitted_bytes: page.emitted_bytes,
                    truncated_by_byte_limit: page.truncated_by_byte_limit,
                }))
            }
            MemoryInspectionInput::Stats {
                contract_code,
                query,
            } => {
                let label = memory_contract_label(&contract_code)?;
                let inspection_path = memory_query_path(query.path);
                Ok(MemoryInspectionOutput::Stats(
                    memory_contract_stats_response(memory, &contract_code, label, &inspection_path)
                        .await?,
                ))
            }
            MemoryInspectionInput::Tree {
                contract_code,
                query,
            } => {
                let label = memory_contract_label(&contract_code)?;
                let target = memory_inspection_target(memory, &contract_code)?;
                let capabilities = target.capabilities();
                let supported = memory_contract_supported(&capabilities);
                let page_request =
                    memory_page_request(query.path, query.cursor, query.limit, query.byte_limit);
                let page = if capabilities.list_tree {
                    target.list_tree(page_request).await?
                } else {
                    empty_memory_tree_page(page_request)
                };
                Ok(MemoryInspectionOutput::Tree(MemoryTreeResponse {
                    contract_code: contract_code.clone(),
                    label: label.to_string(),
                    provider_code: memory.provider_codes.get(&contract_code).cloned(),
                    capabilities: capabilities.into(),
                    supported,
                    inspection_path: page.inspection_path,
                    nodes: page
                        .nodes
                        .into_iter()
                        .map(to_memory_tree_node_response)
                        .collect(),
                    next_cursor: page.next_cursor,
                    limit: page.limit,
                    byte_limit: page.byte_limit,
                    emitted_bytes: page.emitted_bytes,
                    truncated_by_byte_limit: page.truncated_by_byte_limit,
                }))
            }
            MemoryInspectionInput::Search {
                contract_code,
                query,
            } => {
                let label = memory_contract_label(&contract_code)?;
                let target = memory_inspection_target(memory, &contract_code)?;
                let capabilities = target.capabilities();
                let supported = memory_contract_supported(&capabilities);
                let page_request =
                    memory_page_request(query.path, query.cursor, query.limit, query.byte_limit);
                let page = if capabilities.search_entries {
                    target.search_entry_page(&query.q, page_request).await?
                } else {
                    empty_memory_entry_page(page_request)
                };
                Ok(MemoryInspectionOutput::Entries(MemoryEntriesResponse {
                    contract_code: contract_code.clone(),
                    label: label.to_string(),
                    provider_code: memory.provider_codes.get(&contract_code).cloned(),
                    capabilities: capabilities.into(),
                    supported,
                    inspection_path: page.inspection_path,
                    entries: page
                        .entries
                        .into_iter()
                        .map(to_memory_entry_metadata_response)
                        .collect(),
                    next_cursor: page.next_cursor,
                    limit: page.limit,
                    byte_limit: page.byte_limit,
                    emitted_bytes: page.emitted_bytes,
                    truncated_by_byte_limit: page.truncated_by_byte_limit,
                }))
            }
            MemoryInspectionInput::Reveal {
                contract_code,
                body,
            } => {
                memory_contract_label(&contract_code)?;
                let target = memory_inspection_target(memory, &contract_code)?;
                let capabilities = target.capabilities();
                if !capabilities.reveal_value {
                    return Err(
                        ControlPlaneError::InvalidInput("memory_inspection_unsupported").into(),
                    );
                }
                let reveal_mode = parse_memory_reveal_mode(body.reveal_mode.as_deref())?;
                let value = target
                    .reveal_entry(&body.entry_ref, reveal_mode)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("memory_entry"))?;
                self.0
                    .audit_policy
                    .append_reveal_audit(
                        actor,
                        serde_json::json!({
                            "contract_code": value.metadata.contract_code.clone(),
                            "group_code": value.metadata.group_code.clone(),
                            "entry_ref": value.metadata.entry_ref.clone(),
                            "key": value.metadata.key.clone(),
                            "inspection_path": value.metadata.inspection_path.clone(),
                            "entry_kind": value.metadata.entry_kind.clone(),
                            "status": value.metadata.status.clone(),
                            "owner": value.metadata.owner.clone(),
                            "value_size_bytes": value.metadata.value_size_bytes,
                            "reveal_mode": format_memory_reveal_mode(value.reveal_mode),
                            "value_state": format_memory_value_state(value.value_state),
                            "sensitive": value.metadata.sensitive,
                        }),
                    )
                    .await?;
                Ok(MemoryInspectionOutput::Revealed(MemoryEntryValueResponse {
                    metadata: to_memory_entry_metadata_response(value.metadata),
                    reveal_mode: format_memory_reveal_mode(value.reveal_mode),
                    value_state: format_memory_value_state(value.value_state),
                    value: value.value,
                    value_preview: value.value_preview,
                    preview_size_bytes: value.preview_size_bytes,
                    full_value_size_bytes: value.full_value_size_bytes,
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<MemoryInspectionInput, MemoryInspectionOutput>
    for MemoryInspectionAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: MemoryInspectionInput,
    ) -> ConsoleInterfaceFuture<'a, MemoryInspectionOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.overview.get",
        binding_id: "http.console.host-infrastructure.memory.overview.get.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.stats-overview.get",
        binding_id: "http.console.host-infrastructure.memory.stats-overview.get.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory/stats",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.entries.list",
        binding_id: "http.console.host-infrastructure.memory.entries.list.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.stats.get",
        binding_id: "http.console.host-infrastructure.memory.stats.get.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/stats",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.entries.search",
        binding_id: "http.console.host-infrastructure.memory.entries.search.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/search",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.tree.list",
        binding_id: "http.console.host-infrastructure.memory.tree.list.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/tree",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.memory.entry.reveal",
        binding_id: "http.console.host-infrastructure.memory.entry.reveal.v1",
        method: "POST",
        path: "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/reveal",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: MemoryInspectionInterfaceDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-host-infrastructure-memory-inspection",
        "graph:console-host-infrastructure-memory-inspection-v1",
        DECLARATIONS,
        Arc::new(MemoryInspectionAdapter(dependencies)),
    )
}

fn has_registered_simple_operations(
    registry: &ConsoleOperationRegistry,
    actor: &domain::ActorContext,
    policies: &[domain::RoleConsolePolicy],
    operation_ids: &[&str],
) -> bool {
    if actor.is_root {
        return true;
    }
    operation_ids.iter().all(|operation_id| {
        let Some(operation) = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == *operation_id)
        else {
            return false;
        };
        if operation.authorization != ConsoleAuthorization::Simple {
            return false;
        }
        let group = match &operation.policy_group {
            ConsolePolicyGroup::SettingsFeature(feature_id) => {
                domain::ConsolePolicyGroup::settings_feature(feature_id)
            }
            ConsolePolicyGroup::Other(group_id) => domain::ConsolePolicyGroup::other(group_id),
        };
        let (Ok(group), Ok(operation_id)) = (
            group,
            domain::ConsoleOperationId::try_from(operation.operation_id.as_str()),
        ) else {
            return false;
        };
        domain::effective_console_simple_operation(policies, &group, &operation_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use interface_runtime::BindingId;

    struct Unavailable;

    impl MemoryAuditPolicyPort for Unavailable {
        fn can_manage<'a>(
            &'a self,
            _actor: &'a domain::ActorContext,
        ) -> MemoryAuditPolicyFuture<'a, bool> {
            Box::pin(async { Ok(false) })
        }

        fn append_reveal_audit<'a>(
            &'a self,
            _actor: &'a domain::ActorContext,
            _payload: serde_json::Value,
        ) -> MemoryAuditPolicyFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn eil_f11_d1_registry_freezes_memory_inspection_bindings() {
        let memory = MemoryInspectionDependencies {
            session_store: None,
            cache_store: None,
            rate_limit_store: None,
            distributed_lock: None,
            task_queue: None,
            event_bus: None,
            runtime_event_stream: None,
            provider_codes: Default::default(),
        };
        let registry = compile_registry(MemoryInspectionInterfaceDependencies {
            memory,
            audit_policy: Arc::new(Unavailable),
        })
        .unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
