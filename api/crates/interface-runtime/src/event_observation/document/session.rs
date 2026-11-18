use super::registration::{validate_descriptor, Projection};
use super::*;
use crate::{finalization::InvocationFinalization, InterfaceExtensionPoint, PluginIdentity};
use std::sync::Mutex;

struct State {
    context: InterfaceEventContext,
    projection: Option<Projection>,
    // Retain the exact owner snapshot through stream terminal and output projection.
    snapshot: Option<Arc<dyn Any + Send + Sync>>,
    before_started: bool,
    interest: Option<bool>,
    closed: bool,
    input: Option<Value>,
    output: Option<Value>,
    records: Vec<crate::InterfaceObserverRecord>,
    faults: usize,
}
/// Invocation-scoped observation only. It cannot replace or authorize an execution plan.
#[derive(Clone)]
pub struct DocumentOperationSession {
    registration: Arc<dyn ErasedDocumentOperationRegistration>,
    observer: Arc<dyn DocumentOperationObserver>,
    state: Arc<Mutex<State>>,
}
impl std::fmt::Debug for DocumentOperationSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentOperationSession")
            .finish_non_exhaustive()
    }
}
fn identity() -> PluginIdentity {
    PluginIdentity::new("interface-runtime.document-operations").expect("static observer identity")
}
impl DocumentOperationSession {
    pub(crate) fn new(
        registration: Arc<dyn ErasedDocumentOperationRegistration>,
        observer: Arc<dyn DocumentOperationObserver>,
        mut context: InterfaceEventContext,
    ) -> Self {
        let capture =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer.capture_context()));
        let faults = usize::from(capture.is_err());
        context.observer_context = capture
            .ok()
            .flatten()
            .map(crate::hook::InvocationExtensionContext);
        let projection = registration.static_projection();
        Self {
            registration,
            observer,
            state: Arc::new(Mutex::new(State {
                context,
                projection,
                snapshot: None,
                before_started: false,
                interest: None,
                closed: false,
                input: None,
                output: None,
                records: Vec::new(),
                faults,
            })),
        }
    }
    /// Called by the trusted target owner after real resolution/authorization. The registered
    /// selector validates the snapshot's target generation and scope. Selecting identity alone
    /// allows a failure Completion without exposing input. A second selection never replaces it.
    pub fn select(&self, snapshot: Arc<dyn Any + Send + Sync>) -> Result<(), Arc<str>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.projection.is_some() || state.before_started {
            state.faults += 1;
            return Err(Arc::from("document-operation-already-frozen"));
        }
        let projection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.registration.select(snapshot.as_ref())
        }));
        let projection = match projection {
            Ok(Ok(projection))
                if validate_descriptor(
                    &projection.descriptor,
                    &state.context.binding,
                    self.registration.generation(),
                ) =>
            {
                projection
            }
            _ => {
                state.faults += 1;
                return Err(Arc::from("document-operation-selection-rejected"));
            }
        };
        state.projection = Some(projection);
        state.snapshot = Some(snapshot);
        Ok(())
    }
    pub(crate) fn is_static(&self) -> bool {
        self.registration.static_descriptor().is_some()
    }
    pub(crate) fn update_attempt(&self, attempt: Option<crate::ExecutionAttempt>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !state.closed {
            state.context.attempt = attempt;
        }
    }
    async fn check_interest(
        &self,
        budget: &mut InvocationFinalization,
        point: InterfaceExtensionPoint,
    ) -> bool {
        let (context, descriptor) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(interested) = state.interest {
                return interested;
            }
            let Some(projection) = &state.projection else {
                return false;
            };
            let descriptor = projection.descriptor.clone();
            // Fail closed if business cancellation drops this check while it is awaiting.
            state.interest = Some(false);
            (state.context.clone(), descriptor)
        };
        let mut interested = false;
        let mut failed = false;
        let token = budget
            .observe(&identity(), point, || async {
                match self.observer.interested(&context, &descriptor).await {
                    Ok(value) => interested = value,
                    Err(_) => failed = true,
                }
            })
            .await;
        if failed {
            budget.record_reported_failure(token);
        }
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .interest = Some(interested);
        interested
    }

    /// Actual post typed-Before input. Dynamic owners call this only after their own target
    /// authorization, before side effects. Observation errors are diagnostic, never a veto.
    pub async fn observe_before<I: InterfaceContract>(&self, input: &I) -> Result<(), Arc<str>> {
        let (projection, context) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed || state.before_started {
                state.faults += 1;
                return Err(Arc::from("document-operation-before-already-frozen"));
            }
            let Some(projection) = state.projection.clone() else {
                state.faults += 1;
                return Err(Arc::from("document-operation-not-selected"));
            };
            state.before_started = true;
            (projection, state.context.clone())
        };
        let mut budget = InvocationFinalization::default();
        if !self
            .check_interest(&mut budget, InterfaceExtensionPoint::Before)
            .await
            || projection.descriptor.schemas.request.is_none()
        {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .records
                .extend(budget.records);
            return Ok(());
        }
        let mut failed = false;
        let mut projected = None;
        let token = budget
            .observe(&identity(), InterfaceExtensionPoint::Before, || async {
                match projection.codec.input(input) {
                    Ok(input) => {
                        projected = Some(input.clone());
                        // Store before awaiting: cancellation of the business future must retain it.
                        self.state.lock().unwrap_or_else(|e| e.into_inner()).input =
                            Some(input.clone());
                        failed = self
                            .observer
                            .observe(DocumentOperationEvent::Before {
                                context,
                                descriptor: projection.descriptor,
                                input,
                            })
                            .await
                            .is_err();
                    }
                    Err(_) => failed = true,
                }
            })
            .await;
        if failed {
            budget.record_reported_failure(token);
        }
        let ok = projected.is_some()
            && !failed
            && budget
                .records
                .iter()
                .all(|r| r.status() == crate::InterfaceObserverStatus::Executed);
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .extend(budget.records);
        if ok {
            Ok(())
        } else {
            Err(Arc::from("document-operation-before-failed"))
        }
    }
    pub(crate) async fn project_output<O: InterfaceContract>(
        &self,
        output: &O,
        budget: &mut InvocationFinalization,
    ) {
        if !self
            .check_interest(budget, InterfaceExtensionPoint::After)
            .await
        {
            return;
        }
        let projection = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .projection
            .clone();
        let Some(projection) = projection.filter(|p| p.descriptor.schemas.result.is_some()) else {
            return;
        };
        let mut failed = false;
        let token = budget
            .observe(&identity(), InterfaceExtensionPoint::After, || async {
                match projection.codec.output(output) {
                    Ok(output) => {
                        self.state.lock().unwrap_or_else(|e| e.into_inner()).output = Some(output)
                    }
                    Err(_) => failed = true,
                }
            })
            .await;
        if failed {
            budget.record_reported_failure(token);
        }
    }
    pub(crate) async fn complete(
        &self,
        terminal: InterfaceInvocationTerminal,
        classification: Option<Arc<str>>,
        budget: &mut InvocationFinalization,
    ) {
        let interested = self
            .check_interest(budget, InterfaceExtensionPoint::Completion)
            .await;
        let (event, records, faults, _snapshot) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return;
            }
            state.closed = true;
            let event = state.projection.take().filter(|_| interested).map(|p| {
                DocumentOperationEvent::Completion {
                    context: state.context.clone(),
                    descriptor: p.descriptor,
                    input: state.input.take(),
                    output: if terminal == InterfaceInvocationTerminal::Completed {
                        state.output.take()
                    } else {
                        None
                    },
                    terminal,
                    classification,
                }
            });
            state.input = None;
            state.output = None;
            (
                event,
                std::mem::take(&mut state.records),
                state.faults,
                state.snapshot.take(),
            )
        };
        budget.records.extend(records);
        if faults > 0 {
            let token = budget
                .observe(&identity(), InterfaceExtensionPoint::Completion, || async {
                })
                .await;
            budget.record_reported_failure(token);
        }
        if let Some(event) = event {
            let mut failed = false;
            let token = budget
                .observe(&identity(), InterfaceExtensionPoint::Completion, || async {
                    failed = self.observer.observe(event).await.is_err();
                })
                .await;
            if failed {
                budget.record_reported_failure(token);
            }
        }
    }
}
