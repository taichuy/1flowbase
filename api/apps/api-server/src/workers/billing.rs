use std::sync::Arc;

use async_trait::async_trait;
use control_plane::{
    billing::{dispatch_credit_events, CreditEventPublisher},
    ports::{BillingRepository, CreditOutboxEvent, EventBus},
};
use time::OffsetDateTime;

use crate::app_state::ApiState;

struct HostCreditEventPublisher {
    bus: Arc<dyn EventBus>,
}

#[async_trait]
impl CreditEventPublisher for HostCreditEventPublisher {
    async fn publish(&self, event: &CreditOutboxEvent) -> anyhow::Result<()> {
        self.bus
            .publish(
                "credit.events",
                serde_json::json!({
                    "event_id":event.event_id,"event_type":event.event_type,
                    "workspace_id":event.workspace_id,"account_id":event.account_id,
                    "occurred_at":event.created_at,"payload":event.payload,
                }),
            )
            .await
    }
}

async fn run(state: Arc<ApiState>) {
    let worker_id = format!("{}:billing", state.api_node_id);
    let publisher = HostCreditEventPublisher {
        bus: state.infrastructure.event_bus(),
    };
    let mut event_tick = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut recovery_tick = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        tokio::select! {
            _=event_tick.tick()=>{
                if let Err(error)=dispatch_credit_events(&state.store,&publisher,&worker_id,50).await{
                    tracing::warn!(error=%error,"credit outbox dispatch failed");
                }
            }
            _=recovery_tick.tick()=>{
                match state.store.recover_expired_credit_reservations(OffsetDateTime::now_utc(),50).await{
                    Ok(count) if count>0=>tracing::info!(recovered=count,"expired credit reservations recovered"),
                    Ok(_)=>{},
                    Err(error)=>tracing::warn!(error=%error,"expired credit reservation recovery failed"),
                }
            }
        }
    }
}

pub fn spawn_billing_worker(state: Arc<ApiState>) {
    tokio::spawn(run(state));
}
