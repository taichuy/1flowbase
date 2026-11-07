use super::*;
use crate::{InterfaceEvent, InterfaceEventContext, InterfaceExtensionPoint};

fn observer_identity() -> PluginIdentity {
    PluginIdentity::new("interface-runtime.host-events").expect("static host observer identity")
}

impl ReceiptBuilder {
    fn event_context(&self) -> Option<InterfaceEventContext> {
        let plan = self.frozen_plan.as_ref()?;
        plan.event_observer.as_ref()?;
        Some(InterfaceEventContext {
            principal: self.event_principal.clone(),
            definition: plan.definition().clone(),
            binding: plan.binding().clone(),
            resolved: self.resolved.clone()?,
            invocation_id: self.invocation_id,
            parent_invocation_id: self.parent_invocation_id,
            graph_fingerprint: self.graph_fingerprint.clone(),
            registry_fingerprint: self.registry_fingerprint.clone(),
            attempt: self.attempt.clone(),
            extension_context: self.extension_context.clone(),
        })
    }

    async fn observe_event_before<I: InterfaceContract>(&mut self, input: &I) {
        let Some(context) = self.event_context() else {
            return;
        };
        let observer = self
            .frozen_plan
            .as_ref()
            .and_then(|plan| plan.event_observer.clone())
            .expect("event context requires a frozen observer");
        // Before observation must not start the post-invocation finalization clock.
        let mut budget = InvocationFinalization::default();
        let projected = &mut self.event_input;
        let mut failed = false;
        let token = budget
            .observe(
                &observer_identity(),
                InterfaceExtensionPoint::Before,
                || async {
                    match input.project_for_event() {
                        Some(input) => {
                            *projected = Some(input.clone());
                            failed = observer
                                .observe(InterfaceEvent::Before { context, input })
                                .await
                                .is_err();
                        }
                        None => failed = true,
                    }
                },
            )
            .await;
        if failed {
            budget.record_reported_failure(token);
        }
        self.finalization.records.extend(budget.records);
    }

    async fn project_event_output<O: InterfaceContract>(&mut self, output: &O) {
        if self.event_context().is_none() {
            return;
        }
        let projected = &mut self.event_output;
        let mut failed = false;
        let token = self
            .finalization
            .observe(
                &observer_identity(),
                InterfaceExtensionPoint::After,
                || async {
                    *projected = output.project_for_event();
                    failed = projected.is_none();
                },
            )
            .await;
        if failed {
            self.finalization.record_reported_failure(token);
        }
    }

    async fn observe_event_completion(
        &mut self,
        terminal: InterfaceInvocationTerminal,
        classification: Option<Arc<str>>,
    ) {
        let Some(context) = self.event_context() else {
            return;
        };
        let observer = self
            .frozen_plan
            .as_ref()
            .and_then(|plan| plan.event_observer.clone())
            .expect("event context requires a frozen observer");
        let event = InterfaceEvent::Completion {
            context,
            input: self.event_input.take(),
            output: self.event_output.take(),
            terminal,
            classification,
        };
        crate::event_observation::observe_event(&mut self.finalization, observer.as_ref(), event)
            .await;
    }
}
