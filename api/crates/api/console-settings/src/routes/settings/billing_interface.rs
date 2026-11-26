use crate::openapi_interface::json_contract::{self, JsonApiInput, JsonApiOutput};
mod managed_projection;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use control_plane::{
    billing::PricingRule,
    errors::ControlPlaneError,
    ports::{
        BillingRepository, CacheStore, CreditAccountRecord, CreditCommandInput,
        CreditTransactionRecord, ListCreditLedgerInput, ListPricingRulesInput,
        ListPricingTargetModelsInput, ListPricingTargetProvidersInput, UpsertPricingRuleInput,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::billing::{
    body_to_rule, invalidate_pricing_rules_cache, CreditCommandBody, ImportCatalogBody, PageQuery,
    PricingCatalogPageResponse, PricingCatalogQuery, PricingRuleBody, PricingRuleQuery,
    PricingRuleResponse, PricingRulesPageResponse, PricingTargetProviderResponse,
    PricingTargetResponse, PricingTargetsPageResponse, PricingTargetsQuery,
};
use crate::{
    error_response::ApiError,
    model_pricing_catalog::{install_pricing_rules_if_absent, RemotePricingCatalogReader},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BillingInput {
    #[serde(serialize_with = "json_contract::query")]
    ListPricingRules(PricingRuleQuery),
    #[serde(serialize_with = "json_contract::query")]
    ListPricingTargets(PricingTargetsQuery),
    #[serde(serialize_with = "json_contract::body")]
    CreatePricingRule(PricingRuleBody),
    UpdatePricingRule {
        id: Uuid,
        body: PricingRuleBody,
    },
    DeletePricingRule {
        id: Uuid,
    },
    #[serde(serialize_with = "json_contract::query")]
    GetPricingCatalog(PricingCatalogQuery),
    #[serde(serialize_with = "json_contract::body")]
    ImportPricingCatalog(ImportCatalogBody),
    SyncPricingCatalog,
    #[serde(serialize_with = "json_contract::query")]
    ListCreditAccounts(PageQuery),
    GetCreditAccount {
        user_id: Uuid,
    },
    #[serde(serialize_with = "json_contract::query")]
    ListCreditLedger(PageQuery),
    GrantCredit {
        user_id: Uuid,
        body: CreditCommandBody,
    },
    ChargeCredit {
        user_id: Uuid,
        body: CreditCommandBody,
    },
    AdjustCredit {
        user_id: Uuid,
        body: CreditCommandBody,
    },
    EnableCharge {
        user_id: Uuid,
        body: CreditCommandBody,
    },
    DisableCharge {
        user_id: Uuid,
        body: CreditCommandBody,
    },
    RefundCredit {
        user_id: Uuid,
        body: CreditCommandBody,
    },
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BillingOutput {
    PricingRules(PricingRulesPageResponse),
    PricingTargets(PricingTargetsPageResponse),
    PricingRule(PricingRuleResponse),
    Deleted(Value),
    PricingCatalog(PricingCatalogPageResponse),
    Imported(Value),
    CreditAccounts(Vec<CreditAccountRecord>),
    CreditAccount(Option<CreditAccountRecord>),
    CreditLedger(Vec<CreditTransactionRecord>),
    CreditTransaction(CreditTransactionRecord),
}

pub struct BillingDependencies {
    pub store: MainDurableStore,
    pub cache_store: Arc<dyn CacheStore>,
    pub catalog_index_url: String,
    pub network_egress: crate::network_egress_client::NetworkEgressHttpClientResolver,
}

struct BillingAdapter(BillingDependencies);

pub fn port(
    dependencies: BillingDependencies,
) -> Arc<dyn ConsoleInterfacePort<BillingInput, BillingOutput>> {
    Arc::new(BillingAdapter(dependencies))
}

impl BillingAdapter {
    async fn open_pricing_catalog(
        &self,
        workspace_id: Uuid,
    ) -> Result<
        (
            RemotePricingCatalogReader,
            crate::network_egress_client::NetworkEgressHttpRequestScope,
        ),
        ApiError,
    > {
        let request = self
            .0
            .network_egress
            .resolve_http_request_with_timeouts(
                workspace_id,
                domain::NetworkEgressConsumerSelector::GithubOfficialSources,
                Duration::from_secs(5),
                Duration::from_secs(20),
            )
            .await?;
        match RemotePricingCatalogReader::open(request.http_client(), &self.0.catalog_index_url)
            .await
        {
            Ok(catalog) => Ok((catalog, request)),
            Err(error) => request.finish(Err(error)).await,
        }
    }

    async fn execute_credit_command(
        &self,
        principal: &UserPrincipal,
        user_id: Uuid,
        body: CreditCommandBody,
        command: &'static str,
    ) -> Result<CreditTransactionRecord, ApiError> {
        let actor = principal.actor();
        Ok(self
            .0
            .store
            .execute_credit_command(&CreditCommandInput {
                workspace_id: actor.current_workspace_id,
                user_id,
                amount: body.amount.unwrap_or_else(|| "0".into()),
                credit_unit: "USD".into(),
                command: command.into(),
                reason: body.reason,
                source_type: body.source_type,
                source_id: body.source_id,
                idempotency_key: body.idempotency_key,
                actor_user_id: Some(actor.user_id),
                actor_plugin_id: None,
                metadata: body.metadata.unwrap_or_else(|| serde_json::json!({})),
            })
            .await?)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: BillingInput,
    ) -> Result<BillingOutput, ApiError> {
        let actor = principal.actor();
        match input {
            BillingInput::ListPricingRules(query) => {
                let page = query.page.unwrap_or(1).max(1);
                let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
                let result = self
                    .0
                    .store
                    .list_pricing_rules(&ListPricingRulesInput {
                        provider_code: query.provider_code,
                        upstream_model_id: query.upstream_model_id,
                        enabled: query.enabled,
                        source_kind: query.source_kind,
                        page_size,
                        offset: (page - 1) * page_size,
                    })
                    .await?;
                Ok(BillingOutput::PricingRules(PricingRulesPageResponse {
                    items: result.items.into_iter().map(Into::into).collect(),
                    total_count: result.total_count,
                    page,
                    page_size,
                }))
            }
            BillingInput::ListPricingTargets(query) => {
                let page = query.page.unwrap_or(1).max(1);
                let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
                let offset = (page - 1) * page_size;
                let now = time::OffsetDateTime::now_utc();
                let provider_code = query
                    .provider_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let Some(provider_code) = provider_code else {
                    let provider_query = query
                        .provider_query
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned);
                    let result = self
                        .0
                        .store
                        .list_pricing_target_providers(&ListPricingTargetProvidersInput {
                            provider_query,
                            at: now,
                            page_size,
                            offset,
                        })
                        .await?;
                    return Ok(BillingOutput::PricingTargets(PricingTargetsPageResponse {
                        providers: result
                            .items
                            .into_iter()
                            .map(|item| PricingTargetProviderResponse {
                                provider_code: item.provider_code,
                                model_count: item.model_count,
                            })
                            .collect(),
                        providers_total_count: result.total_count,
                        items: Vec::new(),
                        total_count: 0,
                        page,
                        page_size,
                        selected_target: None,
                    }));
                };

                let upstream_model_id_query = query
                    .upstream_model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let result = self
                    .0
                    .store
                    .list_pricing_target_models(&ListPricingTargetModelsInput {
                        provider_code: provider_code.clone(),
                        upstream_model_id_query,
                        at: now,
                        page_size,
                        offset,
                    })
                    .await?;
                let mut grouped = BTreeMap::<String, Vec<PricingRule>>::new();
                for rule in result.rules {
                    grouped
                        .entry(rule.upstream_model_id.clone())
                        .or_default()
                        .push(rule);
                }
                let mut items = Vec::with_capacity(result.model_ids.len());
                for model_id in result.model_ids {
                    let Some(candidate_rules) = grouped.remove(&model_id) else {
                        continue;
                    };
                    let Some(rule) =
                        control_plane::billing::choose_pricing_rule(candidate_rules, now)?
                    else {
                        continue;
                    };
                    items.push(PricingTargetResponse::try_from(rule)?);
                }

                let selected_model_id = query
                    .selected_model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let selected_target = match selected_model_id {
                    None => None,
                    Some(model_id) => {
                        if let Some(found) =
                            items.iter().find(|item| item.upstream_model_id == model_id)
                        {
                            Some(found.clone())
                        } else {
                            let exact_rules = self
                                .0
                                .store
                                .list_pricing_rules_for_targets(
                                    &[(provider_code.clone(), model_id)],
                                    now,
                                )
                                .await?;
                            control_plane::billing::choose_pricing_rule(exact_rules, now)?
                                .map(PricingTargetResponse::try_from)
                                .transpose()?
                        }
                    }
                };

                Ok(BillingOutput::PricingTargets(PricingTargetsPageResponse {
                    providers: Vec::new(),
                    providers_total_count: 0,
                    items,
                    total_count: result.total_count,
                    page,
                    page_size,
                    selected_target,
                }))
            }
            BillingInput::CreatePricingRule(body) => {
                let row = self
                    .0
                    .store
                    .upsert_pricing_rule(&UpsertPricingRuleInput {
                        rule: body_to_rule(body, actor.user_id, None)?,
                    })
                    .await?;
                invalidate_pricing_rules_cache(self.0.cache_store.as_ref(), &row).await?;
                Ok(BillingOutput::PricingRule(row.into()))
            }
            BillingInput::UpdatePricingRule { id, body } => {
                let Some(previous) = self.0.store.get_pricing_rule(id).await? else {
                    return Err(ControlPlaneError::NotFound("pricing_rule").into());
                };
                let row = self
                    .0
                    .store
                    .upsert_pricing_rule(&UpsertPricingRuleInput {
                        rule: body_to_rule(body, actor.user_id, Some(id))?,
                    })
                    .await?;
                invalidate_pricing_rules_cache(self.0.cache_store.as_ref(), &previous).await?;
                invalidate_pricing_rules_cache(self.0.cache_store.as_ref(), &row).await?;
                Ok(BillingOutput::PricingRule(row.into()))
            }
            BillingInput::DeletePricingRule { id } => {
                let previous = self.0.store.get_pricing_rule(id).await?;
                let deleted = self.0.store.delete_pricing_rule(id).await?;
                if let Some(previous) = previous {
                    invalidate_pricing_rules_cache(self.0.cache_store.as_ref(), &previous).await?;
                }
                Ok(BillingOutput::Deleted(
                    serde_json::json!({"deleted": deleted}),
                ))
            }
            BillingInput::GetPricingCatalog(query) => {
                let (catalog, request) = self
                    .open_pricing_catalog(actor.current_workspace_id)
                    .await?;
                let page = query.page.unwrap_or(1).max(1);
                let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
                let result = if query.metadata_only.unwrap_or(false) {
                    catalog
                        .count(
                            query.provider_code.as_deref(),
                            query.upstream_model_id.as_deref(),
                        )
                        .await
                        .map(|total| (Vec::new(), total))
                } else {
                    catalog
                        .query(
                            query.provider_code.as_deref(),
                            query.upstream_model_id.as_deref(),
                            page,
                            page_size,
                        )
                        .await
                };
                let (items, total_count) = request.finish(result).await?;
                Ok(BillingOutput::PricingCatalog(PricingCatalogPageResponse {
                    schema_version: "1flowbase.model-pricing-page/v2",
                    catalog_version: catalog.catalog_version().to_owned(),
                    currency_code: "USD",
                    items,
                    total_count,
                    page,
                    page_size,
                }))
            }
            BillingInput::SyncPricingCatalog => {
                let (catalog, request) = self
                    .open_pricing_catalog(actor.current_workspace_id)
                    .await?;
                let staged: Result<
                    Box<dyn control_plane::ports::OfficialPricingCatalogSync>,
                    ApiError,
                > = async {
                    let mut session = self.0.store.begin_official_pricing_sync().await?;
                    for first in (1..=catalog.page_count()).step_by(4) {
                        let mut requests = Vec::new();
                        for number in first..=first.saturating_add(3).min(catalog.page_count()) {
                            requests.push(catalog.read_page(number));
                        }
                        // Keep at most four remote pages in flight and stage each independently.
                        for page in futures_util::future::try_join_all(requests).await? {
                            let rules = page
                                .into_iter()
                                .map(|body| body_to_rule(body, actor.user_id, None))
                                .collect::<Result<Vec<_>, _>>()?;
                            session.stage(&rules).await?;
                        }
                    }
                    Ok(session)
                }
                .await;
                // Release the egress owner before committing; release failures also discard staging.
                let summary = request.finish(staged).await?.finish().await?;
                // Also invalidate on a retry after a previous cache invalidation failure.
                self.0
                    .cache_store
                    .clear_cache_domain("model-pricing")
                    .await?;
                Ok(BillingOutput::Imported(serde_json::to_value(summary)?))
            }
            BillingInput::ImportPricingCatalog(body) => {
                let (catalog, request) = self
                    .open_pricing_catalog(actor.current_workspace_id)
                    .await?;
                let selected = request
                    .finish(catalog.select_ids(&body.catalog_ids).await)
                    .await?;
                let summary =
                    install_pricing_rules_if_absent(&self.0.store, actor.user_id, selected).await?;
                if summary.inserted > 0 {
                    self.0
                        .cache_store
                        .clear_cache_domain("model-pricing")
                        .await?;
                }
                Ok(BillingOutput::Imported(serde_json::to_value(summary)?))
            }
            BillingInput::ListCreditAccounts(query) => Ok(BillingOutput::CreditAccounts(
                self.0
                    .store
                    .list_credit_accounts(
                        actor.current_workspace_id,
                        query.limit.unwrap_or(100),
                        query.offset.unwrap_or(0),
                    )
                    .await?,
            )),
            BillingInput::GetCreditAccount { user_id } => Ok(BillingOutput::CreditAccount(
                self.0
                    .store
                    .get_credit_account(actor.current_workspace_id, user_id)
                    .await?,
            )),
            BillingInput::ListCreditLedger(query) => Ok(BillingOutput::CreditLedger(
                self.0
                    .store
                    .list_credit_ledger(&ListCreditLedgerInput {
                        workspace_id: actor.current_workspace_id,
                        user_id: query.user_id,
                        before_created_at: query.before_created_at,
                        before_id: query.before_id,
                        limit: query.limit.unwrap_or(100),
                    })
                    .await?,
            )),
            BillingInput::GrantCredit { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "grant")
                    .await?,
            )),
            BillingInput::ChargeCredit { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "charge")
                    .await?,
            )),
            BillingInput::AdjustCredit { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "adjustment")
                    .await?,
            )),
            BillingInput::EnableCharge { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "enable_charge")
                    .await?,
            )),
            BillingInput::DisableCharge { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "disable_charge")
                    .await?,
            )),
            BillingInput::RefundCredit { user_id, body } => Ok(BillingOutput::CreditTransaction(
                self.execute_credit_command(principal, user_id, body, "refund")
                    .await?,
            )),
        }
    }
}

impl ConsoleInterfacePort<BillingInput, BillingOutput> for BillingAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: BillingInput,
    ) -> ConsoleInterfaceFuture<'a, BillingOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_rules.list",
        binding_id: "http.console.settings.billing.pricing-rules.list.v1",
        method: "GET",
        path: "/api/console/settings/billing/pricing-rules",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_targets.list",
        binding_id: "http.console.settings.billing.pricing-targets.list.v1",
        method: "GET",
        path: "/api/console/settings/billing/pricing-targets",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_rules.create",
        binding_id: "http.console.settings.billing.pricing-rules.create.v1",
        method: "POST",
        path: "/api/console/settings/billing/pricing-rules",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_rules.update",
        binding_id: "http.console.settings.billing.pricing-rules.update.v1",
        method: "PATCH",
        path: "/api/console/settings/billing/pricing-rules/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_rules.delete",
        binding_id: "http.console.settings.billing.pricing-rules.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/billing/pricing-rules/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_catalog.view",
        binding_id: "http.console.settings.billing.pricing-catalog.get.v1",
        method: "GET",
        path: "/api/console/settings/billing/pricing-catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_catalog.import",
        binding_id: "http.console.settings.billing.pricing-catalog.import.v1",
        method: "POST",
        path: "/api/console/settings/billing/pricing-catalog/import",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.pricing_catalog.sync",
        binding_id: "http.console.settings.billing.pricing-catalog.sync.v1",
        method: "POST",
        path: "/api/console/settings/billing/pricing-catalog/sync",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit_accounts.list",
        binding_id: "http.console.settings.billing.credit-accounts.list.v1",
        method: "GET",
        path: "/api/console/settings/billing/credit-accounts",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit_accounts.view",
        binding_id: "http.console.settings.billing.credit-accounts.get.v1",
        method: "GET",
        path: "/api/console/settings/billing/credit-accounts/:user_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit_ledger.list",
        binding_id: "http.console.settings.billing.credit-ledger.list.v1",
        method: "GET",
        path: "/api/console/settings/billing/credit-ledger",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.grant",
        binding_id: "http.console.settings.billing.credits.grant.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/grant",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.charge",
        binding_id: "http.console.settings.billing.credits.charge.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/charge",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.adjust",
        binding_id: "http.console.settings.billing.credits.adjust.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/adjust",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.enable",
        binding_id: "http.console.settings.billing.credits.enable.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/enable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.disable",
        binding_id: "http.console.settings.billing.credits.disable.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/disable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "billing.credit.refund",
        binding_id: "http.console.settings.billing.credits.refund.v1",
        method: "POST",
        path: "/api/console/settings/billing/credits/:user_id/refund",
        mutating: true,
    },
];

pub fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<BillingInput, BillingOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-billing",
        "graph:console-billing-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableBillingPort;

#[cfg(test)]
impl ConsoleInterfacePort<BillingInput, BillingOutput> for UnavailableBillingPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: BillingInput,
    ) -> ConsoleInterfaceFuture<'a, BillingOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("billing fixture unavailable").into(),
            ))
        })
    }
}

impl JsonApiInput for BillingInput {}
impl JsonApiOutput for BillingOutput {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f13b_registry_freezes_billing_bindings() {
        let registry = compile_registry(Arc::new(UnavailableBillingPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared billing binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
