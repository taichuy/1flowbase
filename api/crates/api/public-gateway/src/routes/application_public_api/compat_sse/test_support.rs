use super::*;
#[path = "tests/support.rs"]
pub mod support;

pub use super::{
    callback_attachment, callback_execution, callback_reservation, durable_replay,
    event_forwarding, protocol_mappers, responses_projection,
};

pub use super::callback_attachment::*;

pub use super::callback_execution::*;

pub use super::callback_reservation::*;

pub use super::event_forwarding::*;

pub use super::protocol_mappers::*;

pub use super::durable_replay::*;

pub use super::event_forwarding::terminal_fallback::*;

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl CompatibleTypedTurnStream {
    pub fn initial_run_for_test_support(&self) -> &NativeRunResult {
        &self.initial_run
    }
    pub fn events_for_test_support(&mut self) -> &mut mpsc::Receiver<CompatibleProjectionInput> {
        &mut self.events
    }
    pub fn into_events_for_test_support(self) -> mpsc::Receiver<CompatibleProjectionInput> {
        self.events
    }
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl CompatibleResumePlan {
    pub fn initial_run_for_test_support(&self) -> &NativeRunResult {
        &self.initial_run
    }
    pub fn command_for_test_support(&self) -> &ResumePublishedCallbackCommand {
        &self.command
    }
    pub fn reservation_for_test_support(&self) -> &Option<CompatibleCallbackReservation> {
        &self.reservation
    }
    pub fn into_parts_for_test_support(
        self,
    ) -> (
        NativeRunResult,
        ResumePublishedCallbackCommand,
        Option<CompatibleCallbackReservation>,
    ) {
        (self.initial_run, self.command, self.reservation)
    }
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl CompatibleProjectionInput {
    pub fn envelope_for_test_support(&self) -> &RuntimeEventEnvelope {
        &self.envelope
    }
    pub fn delivery_for_test_support(&self) -> &Option<RuntimeEventDeliveryReceipt> {
        &self.delivery
    }
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl OpenedCompatibleTurn {
    pub fn initial_run_for_test_support(&self) -> &NativeRunResult {
        &self.initial_run
    }
    pub fn delivery_scope_for_test_support(&self) -> RuntimeEventDeliveryScope {
        self.delivery_scope
    }
    pub fn ignored_waiting_callback_task_id_for_test_support(&self) -> Option<uuid::Uuid> {
        self.ignored_waiting_callback_task_id
    }
    pub fn projection_for_test_support(&self) -> &CompatibleTurnProjection {
        &self.projection
    }
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl CompatibleStreamStats {
    pub fn emitted_content_bytes_for_test_support(&self) -> usize {
        self.emitted_content_bytes
    }
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub struct CompatibleReplayTestSupport {
    pub from_sequence: Option<i64>,
    pub durable_round_replay: Vec<RuntimeEventEnvelope>,
    pub ignored_waiting_callback_task_id: Option<uuid::Uuid>,
}

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl SubscribedCompatibleTypedEventStream {
    pub fn for_test_support(
        terminal_dependencies: NativeRunTerminalDependencies,
        initial_run: NativeRunResult,
        delivery_policy: CompatibleDeliveryPolicy,
        delivery_scope: RuntimeEventDeliveryScope,
        replay: CompatibleReplayTestSupport,
        subscription: control_plane::ports::RuntimeEventSubscription,
        sender: mpsc::Sender<CompatibleProjectionInput>,
    ) -> Self {
        Self {
            terminal_dependencies,
            initial_run,
            delivery_policy,
            delivery_scope,
            from_sequence: replay.from_sequence,
            durable_round_replay: replay.durable_round_replay,
            ignored_waiting_callback_task_id: replay.ignored_waiting_callback_task_id,
            subscription,
            sender,
        }
    }
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
impl CompatibleProjectionInput {
    pub fn for_test_support(
        run_snapshot: Arc<NativeRunResult>,
        envelope: RuntimeEventEnvelope,
        delivery: Option<RuntimeEventDeliveryReceipt>,
    ) -> Self {
        Self {
            run_snapshot,
            envelope,
            delivery,
        }
    }
}

pub const PROVIDER_UPSTREAM_ERROR_BODY: &str =
    " {\"future_error\":{\"shape\":\"unknown\"},\"message\":\"keep complete body\"}\n ";
