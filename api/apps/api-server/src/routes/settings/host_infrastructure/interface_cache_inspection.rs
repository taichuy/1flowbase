use std::{future::Future, pin::Pin, sync::Arc};

use access_control::ConsoleOperationRegistry;
use control_plane::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{AuthRepository, CacheStore, RoleConsolePolicyReader},
};
use interface_runtime::InterfaceContract;
use storage_durable_postgres::MainDurableStore;

use super::{
    has_registered_simple_operations, to_cache_domain_response, to_cache_entry_metadata_response,
    CacheEntriesResponse, CacheEntryKeyBody, CacheEntryValueResponse, CacheOverviewResponse,
    ClearCacheDomainResponse, ClearCacheEntryResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum CacheInspectionInput {
    Overview,
    Entries {
        domain_code: String,
    },
    Reveal {
        domain_code: String,
        body: CacheEntryKeyBody,
    },
    ClearEntry {
        domain_code: String,
        body: CacheEntryKeyBody,
    },
    ClearDomain {
        domain_code: String,
    },
}

impl InterfaceContract for CacheInspectionInput {
    const CONTRACT_ID: &'static str = "console-host-infrastructure-cache-inspection-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum CacheInspectionOutput {
    Overview(CacheOverviewResponse),
    Entries(CacheEntriesResponse),
    Revealed(CacheEntryValueResponse),
    EntryCleared(ClearCacheEntryResponse),
    DomainCleared(ClearCacheDomainResponse),
}

impl InterfaceContract for CacheInspectionOutput {
    const CONTRACT_ID: &'static str = "console-host-infrastructure-cache-inspection-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct CacheInspectionDependencies {
    pub(crate) cache: Arc<dyn CacheStore>,
    pub(crate) provider_code: Option<String>,
    pub(crate) audit_policy: Arc<dyn CacheAuditPolicyPort>,
}

pub(crate) type CacheAuditPolicyFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send + 'a>>;

pub(crate) trait CacheAuditPolicyPort: Send + Sync {
    fn can_manage<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> CacheAuditPolicyFuture<'a, bool>;

    fn append_audit<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        event_code: &'static str,
        payload: serde_json::Value,
    ) -> CacheAuditPolicyFuture<'a, ()>;
}

pub(crate) struct CacheAuditPolicyAdapter {
    store: MainDurableStore,
    console_registry: Arc<ConsoleOperationRegistry>,
}

impl CacheAuditPolicyAdapter {
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

impl CacheAuditPolicyPort for CacheAuditPolicyAdapter {
    fn can_manage<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> CacheAuditPolicyFuture<'a, bool> {
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
                &[
                    "host_infrastructure.cache.reveal",
                    "host_infrastructure.cache.entry.clear",
                    "host_infrastructure.cache.domain.clear",
                ],
            ))
        })
    }

    fn append_audit<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        event_code: &'static str,
        payload: serde_json::Value,
    ) -> CacheAuditPolicyFuture<'a, ()> {
        Box::pin(async move {
            let workspace_id = (actor.current_workspace_id != domain::SYSTEM_SCOPE_ID)
                .then_some(actor.current_workspace_id);
            AuthRepository::append_audit_log(
                &self.store,
                &audit_log(
                    workspace_id,
                    Some(actor.user_id),
                    "host_infrastructure_cache",
                    None,
                    event_code,
                    payload,
                ),
            )
            .await?;
            Ok(())
        })
    }
}

struct CacheInspectionAdapter(CacheInspectionDependencies);

impl CacheInspectionAdapter {
    async fn execute_inner(
        &self,
        principal: &interface_runtime::UserPrincipal,
        input: CacheInspectionInput,
    ) -> Result<CacheInspectionOutput, ApiError> {
        let actor = principal.actor();
        let cache = &self.0.cache;
        match input {
            CacheInspectionInput::Overview => {
                let capabilities = cache.inspection_capabilities();
                let domains = if capabilities.list_domains {
                    cache
                        .list_cache_domains()
                        .await?
                        .into_iter()
                        .map(to_cache_domain_response)
                        .collect()
                } else {
                    Vec::new()
                };
                Ok(CacheInspectionOutput::Overview(CacheOverviewResponse {
                    provider_code: self.0.provider_code.clone(),
                    can_manage: self.0.audit_policy.can_manage(actor).await?,
                    capabilities: capabilities.into(),
                    domains,
                }))
            }
            CacheInspectionInput::Entries { domain_code } => {
                let capabilities = cache.inspection_capabilities();
                let entries = if capabilities.list_entries {
                    cache
                        .list_cache_entries(&domain_code)
                        .await?
                        .into_iter()
                        .map(to_cache_entry_metadata_response)
                        .collect()
                } else {
                    Vec::new()
                };
                Ok(CacheInspectionOutput::Entries(CacheEntriesResponse {
                    domain_code,
                    capabilities: capabilities.into(),
                    entries,
                }))
            }
            CacheInspectionInput::Reveal { domain_code, body } => {
                let capabilities = cache.inspection_capabilities();
                if !capabilities.reveal_value {
                    return Err(
                        ControlPlaneError::InvalidInput("cache_inspection_unsupported").into(),
                    );
                }
                let value = cache
                    .reveal_cache_entry(&domain_code, &body.key)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("cache_entry"))?;
                self.0
                    .audit_policy
                    .append_audit(
                        actor,
                        "host_infrastructure.cache_value_revealed",
                        serde_json::json!({
                            "domain_code": domain_code,
                            "key": body.key,
                            "value_size_bytes": value.metadata.value_size_bytes,
                        }),
                    )
                    .await?;
                Ok(CacheInspectionOutput::Revealed(CacheEntryValueResponse {
                    metadata: to_cache_entry_metadata_response(value.metadata),
                    value: value.value,
                }))
            }
            CacheInspectionInput::ClearEntry { domain_code, body } => {
                let capabilities = cache.inspection_capabilities();
                if !capabilities.clear_entry {
                    return Err(
                        ControlPlaneError::InvalidInput("cache_inspection_unsupported").into(),
                    );
                }
                let cleared = cache.clear_cache_entry(&domain_code, &body.key).await?;
                self.0
                    .audit_policy
                    .append_audit(
                        actor,
                        "host_infrastructure.cache_entry_cleared",
                        serde_json::json!({
                            "domain_code": domain_code,
                            "key": body.key,
                            "cleared": cleared,
                        }),
                    )
                    .await?;
                Ok(CacheInspectionOutput::EntryCleared(
                    ClearCacheEntryResponse { cleared },
                ))
            }
            CacheInspectionInput::ClearDomain { domain_code } => {
                let capabilities = cache.inspection_capabilities();
                if !capabilities.clear_domain {
                    return Err(
                        ControlPlaneError::InvalidInput("cache_inspection_unsupported").into(),
                    );
                }
                let cleared_count = cache.clear_cache_domain(&domain_code).await?;
                self.0
                    .audit_policy
                    .append_audit(
                        actor,
                        "host_infrastructure.cache_domain_cleared",
                        serde_json::json!({
                            "domain_code": domain_code,
                            "cleared_count": cleared_count,
                        }),
                    )
                    .await?;
                Ok(CacheInspectionOutput::DomainCleared(
                    ClearCacheDomainResponse { cleared_count },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<CacheInspectionInput, CacheInspectionOutput> for CacheInspectionAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: CacheInspectionInput,
    ) -> ConsoleInterfaceFuture<'a, CacheInspectionOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.cache.overview.get",
        binding_id: "http.console.host-infrastructure.cache.overview.get.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/cache",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.cache.entries.list",
        binding_id: "http.console.host-infrastructure.cache.entries.list.v1",
        method: "GET",
        path: "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.cache.entry.reveal",
        binding_id: "http.console.host-infrastructure.cache.entry.reveal.v1",
        method: "POST",
        path: "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/reveal",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.cache.entry.clear",
        binding_id: "http.console.host-infrastructure.cache.entry.clear.v1",
        method: "POST",
        path: "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/clear",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "host_infrastructure.cache.domain.clear",
        binding_id: "http.console.host-infrastructure.cache.domain.clear.v1",
        method: "POST",
        path: "/api/console/settings/host-infrastructure/cache/domains/:domain_code/clear",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: CacheInspectionDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-host-infrastructure-cache-inspection",
        "graph:console-host-infrastructure-cache-inspection-v1",
        DECLARATIONS,
        Arc::new(CacheInspectionAdapter(dependencies)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use interface_runtime::BindingId;

    struct Unavailable;

    impl CacheAuditPolicyPort for Unavailable {
        fn can_manage<'a>(
            &'a self,
            _actor: &'a domain::ActorContext,
        ) -> CacheAuditPolicyFuture<'a, bool> {
            Box::pin(async { Ok(false) })
        }

        fn append_audit<'a>(
            &'a self,
            _actor: &'a domain::ActorContext,
            _event_code: &'static str,
            _payload: serde_json::Value,
        ) -> CacheAuditPolicyFuture<'a, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn eil_f11_d2_registry_freezes_cache_inspection_bindings() {
        let registry = compile_registry(CacheInspectionDependencies {
            cache: Arc::new(storage_ephemeral::MokaCacheStore::new("fixture", 1)),
            provider_code: None,
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
