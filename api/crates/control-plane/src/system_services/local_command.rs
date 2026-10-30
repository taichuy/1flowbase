//! One-shot command validation and audit; interface authorization belongs to the Console Kernel.
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{audit::audit_log, errors::ControlPlaneError, ports::AuthRepository};
use domain::ActorContext;
use uuid::Uuid;

pub type LocalCommandFuture<'a, O> = Pin<Box<dyn Future<Output = anyhow::Result<O>> + Send + 'a>>;

pub trait LocalCommandOutcome: Send + 'static {
    fn audit_payload(&self) -> serde_json::Value;
}

pub trait LocalCommandExecutor: Send + Sync {
    type Output: LocalCommandOutcome;
    fn execute(&self, command: String) -> LocalCommandFuture<'_, Self::Output>;
}

pub struct LocalCommandService<R, E> {
    repository: R,
    executor: Arc<E>,
}

impl<R: AuthRepository, E: LocalCommandExecutor> LocalCommandService<R, E> {
    pub fn new(repository: R, executor: Arc<E>) -> Self {
        Self {
            repository,
            executor,
        }
    }

    pub async fn execute(
        &self,
        actor: &ActorContext,
        command: String,
    ) -> anyhow::Result<E::Output> {
        // The canonical Console operation is the authorization owner. No extra root gate.
        validate_command(&command)?;
        let execution_id = Uuid::now_v7();
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(actor.user_id),
                "local_command",
                Some(execution_id),
                "local_command.started",
                serde_json::json!({"command_bytes": command.len()}),
            ))
            .await?;
        let result = self.executor.execute(command).await;
        let payload = match &result {
            Ok(output) => output.audit_payload(),
            Err(_) => serde_json::json!({"execution_failed": true}),
        };
        if let Err(error) = self
            .repository
            .append_audit_log(&audit_log(
                None,
                Some(actor.user_id),
                "local_command",
                Some(execution_id),
                "local_command.finished",
                payload,
            ))
            .await
        {
            // Execution may already have external effects; an audit failure must not invite a retry.
            tracing::error!(%execution_id, %error, "local command completion audit failed");
        }
        result
    }
}

fn validate_command(command: &str) -> Result<(), ControlPlaneError> {
    if command.trim().is_empty() || command.contains('\0') {
        return Err(ControlPlaneError::InvalidInput("local_command"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/local_command.rs"]
mod tests;
