//! Independent durable command admission, builtin execution and Host response delivery.
use super::{active_binding, actor_for_binding, ApiState, LEASE_SECONDS};
use anyhow::{ensure, Context, Result};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use uuid::Uuid;

mod admission;
mod delivery;
mod effects;

const POLL_INTERVAL: Duration = Duration::from_millis(250);

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), stop: watch::Receiver<bool>) {
    tokio::join!(
        admission::run(state.clone(), key, stop.clone()),
        effects::run(state.clone(), key, stop.clone()),
        delivery::run(state, key, stop),
    );
}

async fn wait(stop: &mut watch::Receiver<bool>) -> bool {
    if *stop.borrow() {
        return false;
    }
    tokio::select! {
        _ = stop.changed() => false,
        _ = tokio::time::sleep(POLL_INTERVAL) => true,
    }
}

#[cfg(all(test, not(feature = "root-1805-consumer-fixture")))]
#[path = "_tests/mod.rs"]
mod tests;
