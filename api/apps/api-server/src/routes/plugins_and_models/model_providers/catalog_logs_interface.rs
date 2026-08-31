use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) struct ProviderLocaleHints {
    explicit: Option<String>,
    accept_language: Option<String>,
}

impl ProviderLocaleHints {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            explicit: headers
                .get("x-1flowbase-locale")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            accept_language: headers
                .get(ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        }
    }

    fn resolve(
        &self,
        query_locale: Option<String>,
        preferred_locale: Option<String>,
    ) -> LocaleMetaResponse {
        runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
            query_locale,
            explicit_header_locale: self.explicit.clone(),
            user_preferred_locale: preferred_locale,
            accept_language: self.accept_language.clone(),
            fallback_locale: runtime_profile::FALLBACK_LOCALE,
            supported_locales: runtime_profile::SUPPORTED_LOCALES
                .iter()
                .map(|value| value.to_string())
                .collect(),
        })
        .into()
    }
}

pub(crate) enum ProviderCatalogLogsInput {
    Catalog {
        query: ModelProviderCatalogQuery,
        locale: ProviderLocaleHints,
    },
    ListLogs(ModelProviderRequestLogsQuery),
    DeleteLogs(DeleteModelProviderRequestLogsBody),
    ClearLogs(serde_json::Value),
}

impl InterfaceContract for ProviderCatalogLogsInput {
    const CONTRACT_ID: &'static str = "console-provider-catalog-logs-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderCatalogLogsOutput {
    Catalog(ModelProviderCatalogResponse),
    Logs(ModelProviderRequestLogsPageResponse),
    Deleted(DeleteModelProviderRequestLogsResponse),
    Cleared(ClearModelProviderRequestLogsResponse),
}

impl InterfaceContract for ProviderCatalogLogsOutput {
    const CONTRACT_ID: &'static str = "console-provider-catalog-logs-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ProviderCatalogLogsAdapter {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    secret_key: String,
    api_node_id: String,
    install_root: String,
    cache_store: Arc<dyn CacheStore>,
}

impl ProviderCatalogLogsAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.provider_runtime.clone()),
            self.secret_key.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation_id,
        )
        .with_node_artifact_context(self.api_node_id.clone(), self.install_root.clone())
        .with_routing_cache_store(self.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderCatalogLogsInput,
    ) -> Result<ProviderCatalogLogsOutput, ApiError> {
        let actor = principal.actor().clone();
        match input {
            ProviderCatalogLogsInput::Catalog { query, locale } => {
                let preferred_locale = self
                    .store
                    .find_user_by_id(actor.user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale_meta = locale.resolve(query.locale, preferred_locale);
                let catalog = self
                    .service(&actor, "model_providers.catalog.view")
                    .list_catalog(actor.user_id, requested_locales(&locale_meta))
                    .await?;
                Ok(ProviderCatalogLogsOutput::Catalog(
                    to_catalog_view_response(locale_meta, catalog),
                ))
            }
            ProviderCatalogLogsInput::ListLogs(query) => {
                let page = self
                    .service(&actor, "model_providers.request_logs.view")
                    .list_request_logs(ListModelProviderRequestLogsCommand {
                        actor,
                        flow_run_id: query.flow_run_id,
                        user_id: query.user_id,
                        application_name: query.application_name,
                        provider_instance_id: query.provider_instance_id,
                        model_id: query.model_id,
                        status: query.status,
                        zero_output_only: query.zero_output_only,
                        started_after: query
                            .started_after
                            .as_deref()
                            .map(parse_rfc3339_time)
                            .transpose()?,
                        started_before: query
                            .started_before
                            .as_deref()
                            .map(parse_rfc3339_time)
                            .transpose()?,
                        page: query.page.unwrap_or(1),
                        page_size: query.page_size.unwrap_or(20),
                    })
                    .await?;
                Ok(ProviderCatalogLogsOutput::Logs(
                    ModelProviderRequestLogsPageResponse {
                        items: page
                            .items
                            .into_iter()
                            .map(to_request_log_response)
                            .collect(),
                        total_count: page.total_count,
                        page: page.page,
                        page_size: page.page_size,
                    },
                ))
            }
            ProviderCatalogLogsInput::DeleteLogs(body) => {
                let attempt_ids = body
                    .attempt_ids
                    .iter()
                    .map(|attempt_id| parse_uuid(attempt_id, "attempt_ids"))
                    .collect::<Result<Vec<_>, _>>()?;
                let deleted_count =
                    self.service(&actor, "model_providers.request_logs.delete")
                        .delete_selected_request_logs(
                            DeleteSelectedModelProviderRequestLogsCommand { actor, attempt_ids },
                        )
                        .await?;
                Ok(ProviderCatalogLogsOutput::Deleted(
                    DeleteModelProviderRequestLogsResponse { deleted_count },
                ))
            }
            ProviderCatalogLogsInput::ClearLogs(body) => {
                let body: ClearModelProviderRequestLogsBody = serde_json::from_value(body)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput("clear_request_logs")
                    })?;
                let workspace_id = actor.current_workspace_id;
                let continuation = match body.continuation_token.as_deref() {
                    Some(token) => ClearModelProviderRequestLogsContinuation::Continue {
                        snapshot_created_before: clear_request_log_continuation::verify(
                            &self.secret_key,
                            workspace_id,
                            token,
                        )?,
                    },
                    None => ClearModelProviderRequestLogsContinuation::Start,
                };
                let result = self
                    .service(&actor, "model_providers.request_logs.clear")
                    .clear_request_logs_batch(ClearModelProviderRequestLogsBatchCommand {
                        actor,
                        continuation,
                    })
                    .await?;
                let continuation_token = clear_request_log_continuation::issue(
                    &self.secret_key,
                    workspace_id,
                    result.snapshot_created_before,
                )?;
                Ok(ProviderCatalogLogsOutput::Cleared(
                    ClearModelProviderRequestLogsResponse {
                        deleted_count: result.deleted_count,
                        has_more: result.has_more,
                        continuation_token,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ProviderCatalogLogsInput, ProviderCatalogLogsOutput>
    for ProviderCatalogLogsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderCatalogLogsInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderCatalogLogsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.catalog.view",
        binding_id: "http.console.model-providers.catalog.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.request-logs.view",
        binding_id: "http.console.model-providers.request-logs.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/request-logs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.request-logs.delete",
        binding_id: "http.console.model-providers.request-logs.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/model-providers/request-logs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.request-logs.clear",
        binding_id: "http.console.model-providers.request-logs.clear.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/request-logs/clear",
        mutating: true,
    },
];

pub(crate) struct ProviderCatalogLogsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

pub(crate) fn compile_registry(
    dependencies: ProviderCatalogLogsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-catalog-logs",
        "graph:console-provider-catalog-logs-v1",
        DECLARATIONS,
        Arc::new(ProviderCatalogLogsAdapter {
            store: dependencies.store,
            provider_runtime: dependencies.provider_runtime,
            secret_key: dependencies.secret_key,
            api_node_id: dependencies.api_node_id,
            install_root: dependencies.install_root,
            cache_store: dependencies.cache_store,
        }),
    )
}
