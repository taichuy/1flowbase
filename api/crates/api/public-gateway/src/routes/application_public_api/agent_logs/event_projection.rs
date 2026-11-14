//! Explicit business fields for all Agent Logs reception operations. No raw facts or credentials.
use interface_runtime::{InterfaceEventContext, InterfaceEventPrincipal, InvocationId};
use serde_json::Value;
use uuid::Uuid;

pub use api_host_kernel::agent_logs_event_projection::*;
pub const OPERATIONS: &[&str] = &[super::ID, super::otlp::ID, CONSOLE_IMPORT_OPERATION];

/// An operation subject, never a native run or an external task terminal.
pub struct ApplicationLogOperationContext {
    pub application_id: Uuid,
    pub workspace_id: Uuid,
    pub invocation_id: InvocationId,
    pub operation: &'static str,
}

/// Only the sealed Application principal can supply public-ingress ownership.
/// Console import retains the route application_id in its admitted typed input.
pub fn application_operation_context(
    context: &InterfaceEventContext,
    input: Option<&Value>,
) -> Option<ApplicationLogOperationContext> {
    let operation = *OPERATIONS
        .iter()
        .find(|operation| **operation == context.definition().interface_id().as_str())?;
    let (application_id, workspace_id) = match (operation, context.principal()?) {
        (CONSOLE_IMPORT_OPERATION, InterfaceEventPrincipal::User(principal)) => (
            input?.get("application_id")?.as_str()?.parse().ok()?,
            principal.actor().current_workspace_id,
        ),
        (super::ID | super::otlp::ID, InterfaceEventPrincipal::Application(principal)) => {
            (principal.application_id(), principal.workspace_id())
        }
        _ => return None,
    };
    Some(ApplicationLogOperationContext {
        application_id,
        workspace_id,
        invocation_id: context.invocation_id(),
        operation,
    })
}
