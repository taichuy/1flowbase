//! System-root policy for one-shot commands in the API host's operating-system environment.
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use crate::{audit::audit_log, errors::ControlPlaneError, ports::AuthRepository};
use domain::ActorContext;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct LocalCommandPolicy {
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

#[derive(Debug)]
pub struct LocalCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub encoding_lossy: bool,
    pub output_complete: bool,
    pub cleanup_complete: bool,
}

pub type LocalCommandFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<LocalCommandOutput>> + Send + 'a>>;

pub trait LocalCommandExecutor: Send + Sync {
    fn execute(&self, command: String, policy: LocalCommandPolicy) -> LocalCommandFuture<'_>;
}

pub struct LocalCommandService<R> {
    repository: R,
    executor: Arc<dyn LocalCommandExecutor>,
    policy: Option<LocalCommandPolicy>,
}

impl<R: AuthRepository> LocalCommandService<R> {
    pub fn new(
        repository: R,
        executor: Arc<dyn LocalCommandExecutor>,
        policy: Option<LocalCommandPolicy>,
    ) -> Self {
        Self {
            repository,
            executor,
            policy,
        }
    }

    pub async fn execute(
        &self,
        actor: &ActorContext,
        command: String,
    ) -> anyhow::Result<LocalCommandOutput> {
        // Interface grants cannot delegate operating-system access to a workspace role.
        authorize(actor, &command)?;
        let policy = self.policy.ok_or(ControlPlaneError::PermissionDenied(
            "local_command_disabled",
        ))?;
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
        let result = self.executor.execute(command, policy).await;
        let payload = match &result {
            Ok(output) => serde_json::json!({
                "exit_code": output.exit_code, "signal": output.signal,
                "duration_ms": output.duration_ms, "timed_out": output.timed_out,
                "truncated": output.truncated, "discarded_bytes": output.discarded_bytes,
                "output_complete": output.output_complete, "cleanup_complete": output.cleanup_complete,
            }),
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

fn authorize(actor: &ActorContext, command: &str) -> Result<(), ControlPlaneError> {
    if !actor.is_root {
        return Err(ControlPlaneError::PermissionDenied(
            "local_command_root_required",
        ));
    }
    if command.trim().is_empty() || command.contains('\0') {
        return Err(ControlPlaneError::InvalidInput("local_command"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/local_command.rs"]
mod tests;
