use std::sync::Arc;

use control_plane::{
    errors::ControlPlaneError,
    ports::{
        BillingRepository, CacheStore, CreditAccountRecord, CreditCommandInput,
        CreditTransactionRecord, ListCreditLedgerInput, ListPricingRulesInput,
        UpsertPricingRuleInput,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::billing::{
    body_to_rule, invalidate_pricing_rules_cache, CreditCommandBody, ImportCatalogBody, PageQuery,
    PricingCatalogPageResponse, PricingCatalogQuery, PricingRuleBody, PricingRuleQuery,
    PricingRuleResponse, PricingRulesPageResponse,
};
use crate::{
    error_response::ApiError,
    model_pricing_catalog::{fetch_remote_pricing_catalog, install_pricing_rules_if_absent},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum BillingInput {
    ListPricingRules(PricingRuleQuery),
    CreatePricingRule(PricingRuleBody),
    UpdatePricingRule {
        id: Uuid,
        body: PricingRuleBody,
    },
    DeletePricingRule {
        id: Uuid,
    },
    GetPricingCatalog(PricingCatalogQuery),
    ImportPricingCatalog(ImportCatalogBody),
    ListCreditAccounts(PageQuery),
    GetCreditAccount {
        user_id: Uuid,
    },
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

impl InterfaceContract for BillingInput {
    const CONTRACT_ID: &'static str = "console-billing-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum BillingOutput {
    PricingRules(PricingRulesPageResponse),
    PricingRule(PricingRuleResponse),
    Deleted(Value),
    PricingCatalog(PricingCatalogPageResponse),
    Imported(Value),
    CreditAccounts(Vec<CreditAccountRecord>),
    CreditAccount(Option<CreditAccountRecord>),
    CreditLedger(Vec<CreditTransactionRecord>),
    CreditTransaction(CreditTransactionRecord),
}

impl InterfaceContract for BillingOutput {
    const CONTRACT_ID: &'static str = "console-billing-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct BillingDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) cache_store: Arc<dyn CacheStore>,
    pub(crate) catalog_index_url: String,
}

struct BillingAdapter(BillingDependencies);

pub(crate) fn port(
    dependencies: BillingDependencies,
) -> Arc<dyn ConsoleInterfacePort<BillingInput, BillingOutput>> {
    Arc::new(BillingAdapter(dependencies))
}

impl BillingAdapter {
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
                let catalog = fetch_remote_pricing_catalog(&self.0.catalog_index_url).await?;
                let provider_filter = query
                    .provider_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let model_filter = query
                    .upstream_model_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let filtered = catalog
                    .rules
                    .into_iter()
                    .filter(|rule| {
                        provider_filter.is_none_or(|filter| {
                            rule.provider_code
                                .to_lowercase()
                                .contains(&filter.to_lowercase())
                        }) && model_filter.is_none_or(|filter| {
                            rule.upstream_model_id
                                .to_lowercase()
                                .contains(&filter.to_lowercase())
                        })
                    })
                    .collect::<Vec<_>>();
                let page = query.page.unwrap_or(1).max(1);
                let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
                let total_count = filtered.len();
                let items = filtered
                    .into_iter()
                    .skip((page - 1) * page_size)
                    .take(page_size)
                    .collect();
                Ok(BillingOutput::PricingCatalog(PricingCatalogPageResponse {
                    schema_version: "1flowbase.model-pricing-page/v1",
                    catalog_version: catalog.catalog_version,
                    currency_code: "USD",
                    items,
                    total_count,
                    page,
                    page_size,
                }))
            }
            BillingInput::ImportPricingCatalog(body) => {
                let catalog = fetch_remote_pricing_catalog(&self.0.catalog_index_url).await?;
                let selected = catalog
                    .rules
                    .into_iter()
                    .filter(|rule| rule.id.is_some_and(|id| body.catalog_ids.contains(&id)))
                    .collect::<Vec<_>>();
                if selected.len() != body.catalog_ids.len() {
                    return Err(ControlPlaneError::InvalidInput("pricing_catalog_id").into());
                }
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

pub(crate) fn compile_registry(
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
