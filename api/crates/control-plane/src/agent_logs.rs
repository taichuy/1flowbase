//! Ingest observations without executing a model or changing credit balances.
use crate::ports::{
    AgentLogsBatch, AgentLogsReceipt, BillingRepository, OrchestrationRuntimeRepository,
};
use anyhow::Result;
use uuid::Uuid;

pub struct AgentLogsService<R> {
    repository: R,
}
impl<R: BillingRepository + OrchestrationRuntimeRepository> AgentLogsService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn ingest(
        &self,
        application_id: Uuid,
        scope_id: Uuid,
        api_key_id: Uuid,
        batch: AgentLogsBatch,
    ) -> Result<AgentLogsReceipt> {
        batch
            .validate()
            .map_err(|_| crate::errors::ControlPlaneError::InvalidInput("agent_logs_batch"))?;
        let mut costs = Vec::with_capacity(batch.events.len());
        for event in &batch.events {
            let mut cost = None;
            if let Some(usage) = &event.usage {
                if !event.inherited {
                    let at = time::OffsetDateTime::parse(
                        &event.occurred_at,
                        &time::format_description::well_known::Rfc3339,
                    )?;
                    if let Some(rule) = crate::billing::resolve_pricing_rule(
                        &self.repository,
                        event.provider_code.as_deref().unwrap_or(""),
                        event.model_id.as_deref().unwrap_or(""),
                        at,
                    )
                    .await?
                    {
                        let all_zero =
                            control_plane_contracts::billing::policy::parse_rules(&rule.rules)?
                                .is_empty()
                                && rule.input_token_unit_price.is_zero()
                                && rule.output_token_unit_price.is_zero()
                                && rule.cache_hit_token_unit_price.is_zero()
                                && rule.cache_write_token_unit_price.is_zero();
                        if all_zero
                            && [
                                usage.total_tokens,
                                usage.input_tokens,
                                usage.output_tokens,
                                usage.input_cache_hit_tokens,
                                usage.cache_write_tokens,
                            ]
                            .into_iter()
                            .any(|v| v.is_some())
                        {
                            cost = Some("0".to_owned());
                        } else if let (
                            Some(input_tokens),
                            Some(output_tokens),
                            Some(input_cache_hit_tokens),
                            Some(cache_write_tokens),
                        ) = (
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.input_cache_hit_tokens,
                            usage.cache_write_tokens,
                        ) {
                            cost = Some(
                                crate::billing::rate_token_usage_at(
                                    &rule,
                                    &crate::billing::TokenUsage {
                                        input_tokens,
                                        output_tokens,
                                        input_cache_hit_tokens,
                                        cache_write_tokens,
                                        ..Default::default()
                                    },
                                    at,
                                )?
                                .total_cost
                                .to_string(),
                            );
                        }
                    }
                }
            }
            costs.push(cost);
        }
        self.repository
            .ingest_agent_logs(application_id, scope_id, api_key_id, &batch, &costs)
            .await
            .map_err(|error| match error.to_string().as_str() {
                "agent_logs.event_conflict" => {
                    crate::errors::ControlPlaneError::Conflict("agent_logs_event_conflict").into()
                }
                "agent_logs.application_type" => {
                    crate::errors::ControlPlaneError::PermissionDenied(
                        "agent_logs_application_type",
                    )
                    .into()
                }
                _ => error,
            })
    }
}
