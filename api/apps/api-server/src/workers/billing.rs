use std::sync::Arc;

use crate::app_state::ApiState;
use control_plane::{billing::dispatch_credit_events, ports::BillingRepository};
use time::OffsetDateTime;

async fn run(state: Arc<ApiState>) {
    let event_bus = state.infrastructure.event_bus();
    let mut event_tick = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut recovery_tick = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        tokio::select! {
            _=event_tick.tick()=>{
                if let Err(error)=dispatch_credit_events(event_bus.as_ref(),50).await{
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
