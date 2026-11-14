use super::*;
use control_plane::application_public_api::callback_resume::{
    PublishedCallbackReservation, PublishedCallbackReservationReceipt,
};
use std::sync::atomic::{AtomicBool, Ordering};

test_support_item! { pub(super), [
]
type CallbackService = ApplicationPublishedCallbackResumeService<
    storage_durable_postgres::MainDurableStore,
    OrchestrationRuntimeService<
        storage_durable_postgres::MainDurableStore,
        crate::provider_runtime::ApiProviderRuntime,
    >,
>;
}

test_support_item! { pub(crate), [
/// Follows the host plan through dispatch. Dropping an unstarted plan must not
/// leave admission processing; the receipt's CAS never releases another owner.
]
struct CompatibleCallbackReservation {
    reservation: PublishedCallbackReservation,
    release_service: Option<Arc<CallbackService>>,
    handed_off: Arc<AtomicBool>,
    previous_generation: Option<Arc<dyn control_plane::ports::RuntimeEventTerminalWriter>>,
}
}

test_support_item! { pub(crate), [
#[derive(Clone)]
]
struct CallbackReservationRelease {
    service: Arc<CallbackService>,
    receipt: PublishedCallbackReservationReceipt,
    handed_off: Arc<AtomicBool>,
}
}

impl CallbackReservationRelease {
    test_support_item! { pub(crate), [
    ]
    async fn release(self) {
        if self.handed_off.load(Ordering::Acquire) {
            return;
        }
        if let Err(error) = self
            .service
            .release_unstarted_native_callback(&self.receipt)
            .await
        {
            warn!(attempt_id = %self.receipt.attempt_id(), %error,
                "failed to release unstarted compatible callback reservation");
        }
    }
    }
}

impl CompatibleCallbackReservation {
    test_support_item! { pub(super), [
    ]
    fn new(
        reservation: PublishedCallbackReservation,
        service: Arc<CallbackService>,
    ) -> Self {
        Self {
            reservation,
            release_service: Some(service),
            handed_off: Arc::new(AtomicBool::new(false)),
            previous_generation: None,
        }
    }
    }

    test_support_item! { pub(super), [
    ]
    async fn capture_previous_generation(
        &mut self,
        stream: &dyn control_plane::ports::RuntimeEventStream,
        run_id: uuid::Uuid,
    ) {
        if self.is_acquired() {
            // A cold process has no hot lane. Capture any existing capability
            // now, before invoker/Kernel awaits can outlive admission ownership.
            self.previous_generation = stream.terminal_writer(run_id).await.ok();
        }
    }
    }

    test_support_item! { pub(super), [
    ]
    fn receipt(&self) -> Option<PublishedCallbackReservationReceipt> {
        match &self.reservation {
            PublishedCallbackReservation::Acquired(receipt) => Some(receipt.clone()),
            _ => None,
        }
    }
    }

    pub(super) fn take_previous_generation(
        &mut self,
    ) -> Option<Arc<dyn control_plane::ports::RuntimeEventTerminalWriter>> {
        self.previous_generation.take()
    }

    test_support_item! { pub(crate), [
    ]
    fn is_acquired(&self) -> bool {
        matches!(self.reservation, PublishedCallbackReservation::Acquired(_))
    }
    }

    test_support_item! { pub(crate), [
    ]
    fn attempt_id(&self) -> uuid::Uuid {
        self.reservation.attempt_id()
    }
    }

    test_support_item! { pub(crate), [
    ]
    fn release_ticket(&self) -> Option<CallbackReservationRelease> {
        match (&self.reservation, &self.release_service) {
            (PublishedCallbackReservation::Acquired(receipt), Some(service)) => {
                Some(CallbackReservationRelease {
                    service: Arc::clone(service),
                    receipt: receipt.clone(),
                    handed_off: Arc::clone(&self.handed_off),
                })
            }
            _ => None,
        }
    }
    }

    test_support_item! { pub(crate), [
    ]
    async fn release(&mut self) {
        if let Some(ticket) = self.release_ticket() {
            ticket.release().await;
        }
        self.release_service = None;
    }
    }

    test_support_item! { pub(super), [
    ]
    fn handoff(&mut self) {
        self.handed_off.store(true, Ordering::Release);
        self.release_service = None;
    }
    }
}

impl Drop for CompatibleCallbackReservation {
    fn drop(&mut self) {
        if let Some(ticket) = self.release_ticket() {
            // This task owns only fenced admission cleanup, never cancellation
            // of an execution. Handoff disarms it before the worker is spawned.
            tokio::spawn(ticket.release());
        }
    }
}
