use super::*;
use crate::billing::{PricingRule, TokenUsage};
use crate::ports::CreditReservation;
use plugin_framework::provider_contract::ProviderUsage;

/// Fee hooks run on the required path; diagnostic log observers consume only their outcome.
pub(super) struct ProviderFeeSubscriber<'a, R, H> {
    invoker: &'a RuntimeProviderInvoker<R, H>,
}

impl<'a, R, H> ProviderFeeSubscriber<'a, R, H>
where
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    pub(super) fn new(invoker: &'a RuntimeProviderInvoker<R, H>) -> Self {
        Self { invoker }
    }

    pub(super) async fn before_invocation(
        &self,
        input: &ProviderInvocationInput,
        pricing_provider_code: &str,
        pricing_model_id: &str,
        billing_node_id: Option<&str>,
    ) -> Result<Option<FeeReservation<R>>> {
        let billing_started_at = OffsetDateTime::now_utc();
        Ok(
            if self
                .invoker
                .repository
                .model_billing_enabled_at(self.invoker.workspace_id)
                .await?
                .is_some_and(|enabled_at| billing_started_at >= enabled_at)
            {
                let actor = self
                    .invoker
                    .flow_execution_context
                    .as_ref()
                    .map(|context| &context.data_model.actor)
                    .ok_or(ControlPlaneError::Conflict(
                        "billing_actor_context_required",
                    ))?;
                let candidates = if let Some(cache) = &self.invoker.model_pricing_cache_store {
                    let key = crate::billing::pricing_rules_cache_key(
                        pricing_provider_code,
                        pricing_model_id,
                    );
                    match cache.get_json(&key).await? {
                        Some(value) => match serde_json::from_value(value) {
                            Ok(rules) => rules,
                            Err(_) => {
                                cache.delete(&key).await?;
                                let rules = self
                                    .invoker
                                    .repository
                                    .model_billing_list_pricing_rules(
                                        pricing_provider_code,
                                        pricing_model_id,
                                    )
                                    .await?;
                                cache
                                    .set_json(
                                        &key,
                                        serde_json::to_value(&rules)?,
                                        Some(time::Duration::minutes(5)),
                                    )
                                    .await?;
                                rules
                            }
                        },
                        None => {
                            let rules = self
                                .invoker
                                .repository
                                .model_billing_list_pricing_rules(
                                    pricing_provider_code,
                                    pricing_model_id,
                                )
                                .await?;
                            cache
                                .set_json(
                                    &key,
                                    serde_json::to_value(&rules)?,
                                    Some(time::Duration::minutes(5)),
                                )
                                .await?;
                            rules
                        }
                    }
                } else {
                    self.invoker
                        .repository
                        .model_billing_match_pricing_rules(
                            pricing_provider_code,
                            pricing_model_id,
                            billing_started_at,
                        )
                        .await?
                };
                let rule = crate::billing::choose_pricing_rule_for(
                    pricing_provider_code,
                    pricing_model_id,
                    candidates,
                    billing_started_at,
                )?
                .ok_or(ControlPlaneError::Conflict("pricing_rule_not_configured"))?;
                let input_tokens = estimate_provider_count_tokens(&input)
                    .map(|estimate| estimate.input_tokens)
                    .unwrap_or(0);
                let maximum_output_tokens = input
                    .model_parameters
                    .get("max_output_tokens")
                    .or_else(|| input.model_parameters.get("max_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let estimate = crate::billing::rate_token_usage(
                    &rule,
                    &crate::billing::TokenUsage {
                        input_tokens: i64::try_from(input_tokens).unwrap_or(i64::MAX),
                        input_cache_hit_tokens: 0,
                        input_cache_miss_tokens: Some(
                            i64::try_from(input_tokens).unwrap_or(i64::MAX),
                        ),
                        output_tokens: i64::try_from(maximum_output_tokens).unwrap_or(i64::MAX),
                        ..Default::default()
                    },
                )?;
                let flow_run_id = self
                    .invoker
                    .flow_run_id
                    .ok_or(ControlPlaneError::Conflict("billing_flow_run_required"))?;
                let invocation_id = billing_invocation_id(flow_run_id, billing_node_id, &input);
                let reservation = self
                    .invoker
                    .repository
                    .model_billing_reserve_credit(&crate::ports::ReserveCreditInput {
                        workspace_id: self.invoker.workspace_id,
                        user_id: actor.user_id,
                        amount: estimate.total_cost.to_string(),
                        flow_run_id: Some(flow_run_id),
                        provider_invocation_id: invocation_id,
                        pricing_rule_id: rule.id,
                        charge_enabled_default: !actor.is_root,
                        reservation_expires_at: billing_started_at + time::Duration::minutes(15),
                    })
                    .await?;
                Some(FeeReservation::new(
                    self.invoker.repository.clone(),
                    rule,
                    reservation,
                    invocation_id,
                    flow_run_id,
                    billing_started_at,
                ))
            } else {
                None
            },
        )
    }
}

pub(super) struct ProviderFeeOutcome<'a> {
    pub upstream_model_id: &'a str,
    pub provider_instance_id: Uuid,
    pub actual_provider_code: &'a str,
    pub invocation_output: &'a mut Option<crate::ports::ProviderRuntimeInvocationOutput>,
    pub invocation_error: &'a mut Option<anyhow::Error>,
    pub canonical_stream_state: Option<&'a CanonicalStreamState>,
    pub native_responses_passthrough: bool,
}

pub(super) struct FeeReservation<R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static>
{
    repository: R,
    rule: PricingRule,
    reservation: CreditReservation,
    invocation_id: Uuid,
    flow_run_id: Uuid,
    started_at: OffsetDateTime,
    heartbeat: tokio::task::JoinHandle<()>,
    armed: bool,
}

impl<R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static> FeeReservation<R> {
    fn new(
        repository: R,
        rule: PricingRule,
        reservation: CreditReservation,
        invocation_id: Uuid,
        flow_run_id: Uuid,
        started_at: OffsetDateTime,
    ) -> Self {
        let heartbeat_repository = repository.clone();
        let session = reservation.billing_session_id;
        let heartbeat = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                match heartbeat_repository
                    .model_billing_heartbeat_credit_reservation(
                        session,
                        OffsetDateTime::now_utc() + time::Duration::minutes(15),
                    )
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(error) => {
                        tracing::warn!(billing_session_id = %session, error = %error, "model billing reservation heartbeat failed")
                    }
                }
            }
        });
        Self {
            repository,
            rule,
            reservation,
            invocation_id,
            flow_run_id,
            started_at,
            heartbeat,
            armed: true,
        }
    }
    async fn release(&mut self, reason: &str) -> Result<()> {
        self.repository
            .model_billing_release_credit(self.reservation.billing_session_id, reason)
            .await?;
        self.disarm();
        Ok(())
    }
    fn disarm(&mut self) {
        self.armed = false;
        self.heartbeat.abort();
    }
}

impl<R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static> Drop for FeeReservation<R> {
    fn drop(&mut self) {
        self.heartbeat.abort();
        if self.armed {
            let repository = self.repository.clone();
            let session = self.reservation.billing_session_id;
            // Forwarder failures, continuation failures and task cancellation retain a cleanup owner.
            tokio::spawn(async move {
                if let Err(error) = repository
                    .model_billing_release_credit(session, "provider_billing_interrupted")
                    .await
                {
                    tracing::error!(billing_session_id = %session, error = %error, "model billing interrupted reservation release failed");
                }
            });
        }
    }
}

fn normalized_token_usage(rule: &PricingRule, usage: &ProviderUsage) -> Result<TokenUsage> {
    let v2 = rule.rating_policy_enabled
        && rule.rating_policy["schema_version"]
            == control_plane_contracts::billing::policy::RATING_POLICY_SCHEMA_V2;
    let convert = |value: u64| {
        if v2 {
            i64::try_from(value).map_err(|_| anyhow!("provider_usage_invalid"))
        } else {
            Ok(i64::try_from(value).unwrap_or(i64::MAX))
        }
    };
    let hits = if v2 {
        usage.input_cache_hit_tokens.or(usage.cache_read_tokens)
    } else {
        usage.input_cache_hit_tokens
    };
    let cache_hit = convert(hits.unwrap_or(0))?;
    let writes = convert(usage.cache_write_tokens.unwrap_or(0))?;
    let miss = usage.input_cache_miss_tokens.map(convert).transpose()?;
    let input_tokens = if v2 {
        match miss {
            Some(ordinary) => ordinary
                .checked_add(cache_hit)
                .and_then(|value| value.checked_add(writes))
                .ok_or_else(|| anyhow!("provider_usage_invalid"))?,
            None => match usage.input_tokens {
                Some(full) => convert(full)?,
                None => cache_hit
                    .checked_add(writes)
                    .ok_or_else(|| anyhow!("provider_usage_invalid"))?,
            },
        }
    } else {
        convert(usage.input_tokens.unwrap_or(0))?
    };
    Ok(TokenUsage {
        input_tokens,
        input_cache_hit_tokens: cache_hit,
        input_cache_miss_tokens: miss,
        output_tokens: convert(usage.output_tokens.unwrap_or(0))?,
        cache_write_tokens: writes,
        cache_write_by_ttl_seconds: usage
            .cache_write_by_ttl_seconds
            .as_ref()
            .map(|buckets| {
                buckets
                    .iter()
                    .map(|(ttl, tokens)| Ok((ttl.clone(), convert(*tokens)?)))
                    .collect::<Result<_>>()
            })
            .transpose()?,
    })
}

fn with_fee_details(
    error: anyhow::Error,
    metadata: Value,
    usage: &ProviderUsage,
    user_account: Option<&str>,
) -> anyhow::Error {
    let mut runtime_error = match error.downcast_ref::<plugin_framework::PluginFrameworkError>() {
        Some(plugin_framework::PluginFrameworkError::RuntimeContract { error }) => {
            (**error).clone()
        }
        _ => ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderUpstreamError,
            error.to_string(),
        ),
    };
    let details = runtime_error
        .provider_details
        .get_or_insert_with(|| json!({}));
    if !details.is_object() {
        *details = json!({"upstream_details":std::mem::take(details)});
    }
    details["_1flowbase_billing"] = metadata;
    details["usage"] = json!(usage);
    if let Some(account) = user_account {
        details["_1flowbase_user_account"] = json!(account);
    }
    plugin_framework::PluginFrameworkError::runtime(runtime_error).into()
}

fn fee_failure(
    code: &str,
    metadata: Value,
    usage: &ProviderUsage,
    user_account: Option<&str>,
) -> anyhow::Error {
    with_fee_details(
        plugin_framework::PluginFrameworkError::runtime(ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse,
            code,
        ))
        .into(),
        metadata,
        usage,
        user_account,
    )
}

impl<R, H> ProviderFeeSubscriber<'_, R, H>
where
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    async fn after_usage(
        &self,
        billing: Option<FeeReservation<R>>,
        outcome: ProviderFeeOutcome<'_>,
    ) -> Result<()> {
        let Some(mut billing) = billing else {
            return Ok(());
        };
        let ProviderFeeOutcome {
            upstream_model_id,
            provider_instance_id,
            actual_provider_code,
            invocation_output,
            invocation_error,
            canonical_stream_state,
            native_responses_passthrough,
        } = outcome;
        let usage = invocation_output.as_ref().map_or_else(
            || {
                canonical_stream_state
                    .map(|state| state.accumulated().usage().value().clone())
                    .unwrap_or_default()
            },
            |output| collected_provider_usage(&output.events, &output.result.usage),
        );
        let has_usage = usage.input_tokens.is_some()
            || usage.input_cache_miss_tokens.is_some()
            || usage.input_cache_hit_tokens.is_some()
            || usage.cache_read_tokens.is_some()
            || usage.cache_write_tokens.is_some()
            || usage.output_tokens.is_some();
        if !has_usage {
            billing
                .release(if invocation_error.is_some() {
                    "provider_invocation_failed_without_usage"
                } else {
                    "provider_usage_unavailable"
                })
                .await?;
            if let Some(error) = invocation_error.take() {
                return Err(error);
            }
            if let Some(output) = invocation_output.as_ref() {
                if orchestration_runtime::execution_engine::billable_provider_output(
                    &output.events,
                    &output.result,
                    native_responses_passthrough,
                ) {
                    let metadata = json!({"billing_status":"reconciliation_failed", "billing_error_code":"provider_usage_unavailable", "billing_session_id":billing.reservation.billing_session_id, "provider_invocation_id":billing.invocation_id, "pricing_rule_id":billing.rule.id});
                    return Err(with_fee_details(
                        provider_usage_unavailable_conflict(output).into(),
                        metadata,
                        &usage,
                        self.invoker
                            .flow_execution_context
                            .as_ref()
                            .and_then(|context| context.user_account.as_deref()),
                    ));
                }
            }
            return Ok(());
        }
        let rated = normalized_token_usage(&billing.rule, &usage)
            .and_then(|normalized| crate::billing::rate_token_usage(&billing.rule, &normalized));
        let rated = match rated {
            Ok(rated) => rated,
            Err(error) => {
                let metadata = json!({"billing_status":"reconciliation_failed", "billing_error_code":"provider_usage_rating_failed", "billing_error":error.to_string(), "billing_session_id":billing.reservation.billing_session_id, "pricing_rule_id":billing.rule.id, "provider_invocation_id":billing.invocation_id, "cache_write_tokens":usage.cache_write_tokens, "cache_write_by_ttl_seconds":usage.cache_write_by_ttl_seconds});
                if let Err(release_error) = billing.release("provider_usage_rating_failed").await {
                    tracing::error!(error = %release_error, "model billing rating failure release failed; cleanup will retry");
                }
                return Err(fee_failure(
                    "provider_usage_rating_failed",
                    metadata,
                    &usage,
                    self.invoker
                        .flow_execution_context
                        .as_ref()
                        .and_then(|context| context.user_account.as_deref()),
                ));
            }
        };
        let rule = &billing.rule;
        let reservation = &billing.reservation;
        let invocation_id = billing.invocation_id;
        let flow_run_id = billing.flow_run_id;
        let billing_started_at = billing.started_at;
        let cache_write_rates = match &rated.applied_rates.cache_write {
            Some(crate::billing::CacheWriteRate::UnitPrice(price)) => {
                json!({"unit_price":price.to_string()})
            }
            Some(crate::billing::CacheWriteRate::ByTtlSeconds(prices)) => {
                json!({"by_ttl_seconds":prices.iter().map(|(ttl, price)| (ttl.clone(), price.to_string())).collect::<std::collections::BTreeMap<_,_>>()})
            }
            None => Value::Null,
        };
        let price_snapshot = json!({
            "pricing_rule_id":rule.id, "pricing_provider_code":rule.provider_code, "pricing_model_id":rule.upstream_model_id,
            "provider_code":actual_provider_code, "upstream_model_id":upstream_model_id, "currency_code":rule.currency_code,
            "request_started_at":billing_started_at,
            "input_token_unit_size":rated.applied_rates.input.unit_size, "input_token_unit_price":rated.applied_rates.input.unit_price.to_string(),
            "output_token_unit_size":rated.applied_rates.output.unit_size, "output_token_unit_price":rated.applied_rates.output.unit_price.to_string(),
            "cache_hit_token_unit_size":rated.applied_rates.cache_hit.unit_size, "cache_hit_token_unit_price":rated.applied_rates.cache_hit.unit_price.to_string(),
            "cache_write":cache_write_rates, "rating_policy_enabled":rule.rating_policy_enabled, "rating_policy":rule.rating_policy,
            "rating_policy_match":rated.rating_policy_match,
        });
        let usage_snapshot = json!({"usage_source":"provider_reported", "ordinary_input_tokens":rated.ordinary_input_tokens,
            "input_cache_hit_tokens":rated.cache_hit_tokens, "cache_write_tokens":rated.cache_write_tokens,
            "cache_write_by_ttl_seconds":usage.cache_write_by_ttl_seconds, "cache_write_cost":rated.cache_write_cost.to_string(),
            "output_tokens":rated.output_tokens, "raw_usage":usage});
        let active_node = self
            .invoker
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.active_node.lock().ok()?.clone());
        let finalized = self
            .invoker
            .repository
            .finalize_model_billing(&crate::ports::FinalizeModelBillingInput {
                usage: crate::ports::AppendUsageLedgerInput {
                    flow_run_id,
                    node_run_id: active_node.as_ref().map(|node| node.node_run_id),
                    span_id: None,
                    failover_attempt_id: None,
                    provider_instance_id: Some(provider_instance_id),
                    gateway_route_id: None,
                    model_id: Some(upstream_model_id.to_string()),
                    upstream_model_id: Some(upstream_model_id.to_string()),
                    upstream_request_id: invocation_output
                        .as_ref()
                        .and_then(|output| output.result.response_id.clone()),
                    input_tokens: usage
                        .input_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cached_input_tokens: usage
                        .input_cache_hit_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    output_tokens: usage
                        .output_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    reasoning_output_tokens: usage
                        .reasoning_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    total_tokens: usage
                        .total_tokens()
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    input_cache_hit_tokens: usage
                        .input_cache_hit_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    input_cache_miss_tokens: usage
                        .input_cache_miss_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cache_read_tokens: usage
                        .cache_read_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cache_write_tokens: usage
                        .cache_write_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    price_snapshot: Some(price_snapshot.clone()),
                    cost_snapshot: Some(
                        json!({"total_cost":rated.total_cost.to_string(),"currency_code":"USD"}),
                    ),
                    usage_status: domain::UsageLedgerStatus::Recorded,
                    raw_usage: serde_json::to_value(&usage)?,
                    normalized_usage: usage_snapshot.clone(),
                },
                cost: crate::ports::AppendCostLedgerInput {
                    flow_run_id: Some(flow_run_id),
                    span_id: None,
                    usage_ledger_id: None,
                    billing_session_id: None,
                    workspace_id: self.invoker.workspace_id,
                    provider_instance_id: Some(provider_instance_id),
                    provider_account_id: None,
                    gateway_route_id: None,
                    model_id: Some(upstream_model_id.to_string()),
                    upstream_model_id: Some(upstream_model_id.to_string()),
                    price_snapshot: price_snapshot.clone(),
                    raw_cost: Some(rated.total_cost.to_string()),
                    normalized_cost: Some(rated.total_cost.to_string()),
                    settlement_currency: Some("USD".to_string()),
                    cost_source: "local_token_pricing".to_string(),
                    cost_status: "rated".to_string(),
                },
                settlement: crate::ports::SettleCreditInput {
                    billing_session_id: reservation.billing_session_id,
                    actual_amount: rated.total_cost.to_string(),
                    cost_ledger_id: None,
                    usage_ledger_id: None,
                    price_snapshot: price_snapshot.clone(),
                    usage_snapshot: usage_snapshot.clone(),
                },
            })
            .await;

        let mut billing_metadata = json!({
            "provider_invocation_id":invocation_id, "billing_session_id":reservation.billing_session_id,
            "pricing_rule_id":rule.id, "pricing_provider_code":rule.provider_code, "pricing_model_id":rule.upstream_model_id,
            "total_cost":rated.total_cost.to_string(), "currency_code":"USD", "charge_skipped":reservation.charge_skipped,
            "cache_write_tokens":rated.cache_write_tokens, "cache_write_by_ttl_seconds":usage.cache_write_by_ttl_seconds,
            "cache_write_cost":rated.cache_write_cost.to_string(),
        });
        match finalized {
            Ok(finalized) => {
                billing_metadata["billing_status"] = json!("settled");
                billing_metadata["usage_ledger_id"] = json!(finalized.usage.id);
                billing_metadata["cost_ledger_id"] = json!(finalized.cost.id);
                billing.disarm();
            }
            Err(error) => {
                tracing::error!(provider_invocation_id = %invocation_id, error = %error, "model billing finalization failed after provider response");
                billing_metadata["billing_status"] = json!("reconciliation_failed");
                billing_metadata["billing_error_code"] = json!("billing_finalize_failed");
                if let Err(release_error) = billing.release("billing_finalize_failed").await {
                    tracing::error!(error = %release_error, "model billing finalization failure release failed; cleanup will retry");
                }
                return Err(fee_failure(
                    "billing_finalize_failed",
                    billing_metadata,
                    &usage,
                    self.invoker
                        .flow_execution_context
                        .as_ref()
                        .and_then(|context| context.user_account.as_deref()),
                ));
            }
        }
        if let Some(error) = invocation_error.take() {
            *invocation_error = Some(with_fee_details(
                error,
                billing_metadata.clone(),
                &usage,
                self.invoker
                    .flow_execution_context
                    .as_ref()
                    .and_then(|context| context.user_account.as_deref()),
            ));
        }
        if let Some(output) = invocation_output.as_mut() {
            let upstream = std::mem::take(&mut output.result.provider_metadata);
            output.result.provider_metadata = json!({"_1flowbase_billing":billing_metadata, "_1flowbase_upstream_provider_metadata":upstream});
        }
        Ok(())
    }
}

pub(super) enum ProviderFeeEvent<
    'a,
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
> {
    BeforeInvocation {
        input: &'a ProviderInvocationInput,
        pricing_provider_code: &'a str,
        pricing_model_id: &'a str,
        billing_node_id: Option<&'a str>,
    },
    AfterUsage {
        reservation: Option<FeeReservation<R>>,
        outcome: ProviderFeeOutcome<'a>,
    },
}

/// This provider-local registration has exactly one required fee subscriber.
/// Log observers are deliberately outside the veto/settlement dispatch path.
pub(super) struct ProviderFeeLifecycle<'a, R, H> {
    subscriber: ProviderFeeSubscriber<'a, R, H>,
}
impl<'a, R, H> ProviderFeeLifecycle<'a, R, H>
where
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    pub(super) fn new(invoker: &'a RuntimeProviderInvoker<R, H>) -> Self {
        Self {
            subscriber: ProviderFeeSubscriber::new(invoker),
        }
    }
    pub(super) async fn dispatch(
        &self,
        event: ProviderFeeEvent<'_, R>,
    ) -> Result<Option<FeeReservation<R>>> {
        match event {
            ProviderFeeEvent::BeforeInvocation {
                input,
                pricing_provider_code,
                pricing_model_id,
                billing_node_id,
            } => {
                self.subscriber
                    .before_invocation(
                        input,
                        pricing_provider_code,
                        pricing_model_id,
                        billing_node_id,
                    )
                    .await
            }
            ProviderFeeEvent::AfterUsage {
                reservation,
                outcome,
            } => {
                self.subscriber.after_usage(reservation, outcome).await?;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
#[path = "_tests/fee_lifecycle_tests.rs"]
mod tests;
