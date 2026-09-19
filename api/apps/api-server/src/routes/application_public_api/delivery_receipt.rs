//! Durable tool delivery receipts.
//!
//! A committed tool event is claimed by the forwarding task, but only the
//! protocol writer knows whether the frame reached the transport. The receipt
//! travels with the projection input to that writer and settles the durable
//! claim from the writer's real outcome:
//!
//! - `projected`: every frame was handed to the transport sender → `acked`.
//! - dropped before any write started → `pending` (provably not projected,
//!   safe to replay on reconnect).
//! - dropped after a write started → `uncertain` (the client may already hold
//!   an executable tool event; recovery is explicit, never automatic replay).

use control_plane::ports::RuntimeEventDeliveryClaim;
use tokio::sync::mpsc;
use tracing::warn;

use super::stream_terminal_fallback::NativeRunTerminalDependencies;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum RuntimeEventDeliveryVerdict {
    Projected,
    Released,
    Uncertain,
}

#[derive(Debug)]
struct RuntimeEventDeliveryOutcome {
    claim: RuntimeEventDeliveryClaim,
    verdict: RuntimeEventDeliveryVerdict,
}

#[derive(Debug)]
pub(crate) struct RuntimeEventDeliveryReceipt {
    claim: Option<RuntimeEventDeliveryClaim>,
    write_started: bool,
    settler: mpsc::UnboundedSender<RuntimeEventDeliveryOutcome>,
}

impl RuntimeEventDeliveryReceipt {
    /// The writer is about to hand the first frame to the transport. From
    /// here on, losing the transport is an uncertain delivery.
    pub(crate) fn begin_write(&mut self) {
        self.write_started = true;
    }

    /// Every frame reached the transport sender.
    pub(crate) fn projected(mut self) {
        self.settle(RuntimeEventDeliveryVerdict::Projected);
    }

    #[cfg(test)]
    pub(crate) fn event_id(&self) -> Option<uuid::Uuid> {
        self.claim.as_ref().map(|claim| claim.event.id)
    }

    fn settle(&mut self, verdict: RuntimeEventDeliveryVerdict) {
        if let Some(claim) = self.claim.take() {
            let _ = self
                .settler
                .send(RuntimeEventDeliveryOutcome { claim, verdict });
        }
    }

    /// A receipt whose verdict is observed by the test instead of the store.
    #[cfg(test)]
    pub(crate) fn probe(
        claim: RuntimeEventDeliveryClaim,
    ) -> (Self, RuntimeEventDeliveryVerdictProbe) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (
            Self {
                claim: Some(claim),
                write_started: false,
                settler: sender,
            },
            RuntimeEventDeliveryVerdictProbe { receiver },
        )
    }
}

#[cfg(test)]
pub(crate) struct RuntimeEventDeliveryVerdictProbe {
    receiver: mpsc::UnboundedReceiver<RuntimeEventDeliveryOutcome>,
}

#[cfg(test)]
impl RuntimeEventDeliveryVerdictProbe {
    pub(crate) async fn verdict(mut self) -> Option<RuntimeEventDeliveryVerdict> {
        self.receiver.recv().await.map(|outcome| outcome.verdict)
    }
}

impl Drop for RuntimeEventDeliveryReceipt {
    fn drop(&mut self) {
        let verdict = if self.write_started {
            RuntimeEventDeliveryVerdict::Uncertain
        } else {
            RuntimeEventDeliveryVerdict::Released
        };
        self.settle(verdict);
    }
}

/// Owns the durable settlement of every receipt it issued. The task keeps
/// running until the last receipt has been settled, independent of the
/// forwarding task that issued it.
pub(crate) struct RuntimeEventDeliverySettler {
    sender: mpsc::UnboundedSender<RuntimeEventDeliveryOutcome>,
    task: tokio::task::JoinHandle<()>,
}

impl RuntimeEventDeliverySettler {
    pub(crate) fn spawn(dependencies: NativeRunTerminalDependencies) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<RuntimeEventDeliveryOutcome>();
        let task = tokio::spawn(async move {
            while let Some(outcome) = receiver.recv().await {
                settle_outcome(&dependencies, outcome).await;
            }
        });
        Self { sender, task }
    }

    pub(crate) fn receipt(&self, claim: RuntimeEventDeliveryClaim) -> RuntimeEventDeliveryReceipt {
        RuntimeEventDeliveryReceipt {
            claim: Some(claim),
            write_started: false,
            settler: self.sender.clone(),
        }
    }

    /// Release an unprojected claim that never became a receipt.
    pub(crate) fn release(&self, claim: RuntimeEventDeliveryClaim) {
        let _ = self.sender.send(RuntimeEventDeliveryOutcome {
            claim,
            verdict: RuntimeEventDeliveryVerdict::Released,
        });
    }

    /// Wait until every issued receipt has been settled.
    pub(crate) async fn settled(self) {
        drop(self.sender);
        if let Err(error) = self.task.await {
            warn!(error = %error, "runtime event delivery settler task failed");
        }
    }
}

async fn settle_outcome(
    dependencies: &NativeRunTerminalDependencies,
    outcome: RuntimeEventDeliveryOutcome,
) {
    let RuntimeEventDeliveryOutcome { claim, verdict } = outcome;
    let result = match verdict {
        RuntimeEventDeliveryVerdict::Projected => {
            dependencies.ack_runtime_event_delivery(&claim).await
        }
        RuntimeEventDeliveryVerdict::Released => {
            dependencies.release_runtime_event_delivery(&claim).await
        }
        RuntimeEventDeliveryVerdict::Uncertain => {
            warn!(
                flow_run_id = %claim.event.flow_run_id,
                runtime_event_id = %claim.event.id,
                event_type = %claim.event.event_type,
                "tool delivery write started but could not be proven; marked uncertain"
            );
            dependencies
                .mark_runtime_event_delivery_uncertain(&claim)
                .await
        }
    };
    if let Err(error) = result {
        warn!(
            flow_run_id = %claim.event.flow_run_id,
            runtime_event_id = %claim.event.id,
            verdict = ?verdict,
            error = %error,
            "failed to settle Responses tool delivery claim"
        );
    }
}
