//! Projection at existing run mutation owners. No observation keeps its own run state.
use super::*;
use control_plane_contracts::ports::{ApplicationLifecycleFact, ApplicationLifecyclePhase};

fn transition_phase(
    previous: Option<domain::FlowRunStatus>,
    current: domain::FlowRunStatus,
) -> Option<ApplicationLifecyclePhase> {
    use domain::FlowRunStatus;
    if current == FlowRunStatus::Running && matches!(previous, None | Some(FlowRunStatus::Queued)) {
        Some(ApplicationLifecyclePhase::Started)
    } else if previous.is_some_and(|status| !status.is_terminal()) && current.is_terminal() {
        Some(ApplicationLifecyclePhase::Ended)
    } else {
        None
    }
}

impl PgControlPlaneStore {
    pub(super) fn application_lifecycle_fact(
        &self,
        workspace_id: Uuid,
        run: &domain::FlowRunRecord,
        previous: Option<domain::FlowRunStatus>,
    ) -> Option<ApplicationLifecycleFact> {
        self.application_lifecycle_observer.as_ref()?;
        let phase = transition_phase(previous, run.status)?;
        Some(ApplicationLifecycleFact {
            phase,
            workspace_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            run_mode: run.run_mode,
            status: run.status,
            occurred_at: match phase {
                ApplicationLifecyclePhase::Started => {
                    if previous.is_none() {
                        run.started_at
                    } else {
                        run.updated_at
                    }
                }
                ApplicationLifecyclePhase::Ended => run.finished_at.unwrap_or(run.updated_at),
            },
            workflow_ancestor_ids: self.workflow_ancestor_ids(),
        })
    }

    pub(super) fn observe_application_lifecycle_fact(
        &self,
        fact: Option<ApplicationLifecycleFact>,
    ) {
        let (Some(observer), Some(fact)) = (&self.application_lifecycle_observer, fact) else {
            return;
        };
        let run_id = fact.flow_run_id;
        // Even a host observation panic must not replace an already committed result.
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer.observe(fact)))
            .is_err()
        {
            tracing::warn!(flow_run_id = %run_id, "application lifecycle observer panicked after commit");
        }
    }

    /// The row lock serializes legacy unconditional writers with terminal CAS owners too.
    pub(super) async fn application_lifecycle_previous_status(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<Option<(Uuid, domain::FlowRunStatus)>> {
        if self.application_lifecycle_observer.is_none() {
            return Ok(None);
        }
        let row =
            sqlx::query("select scope_id,status from flow_runs where id=$1 for no key update")
                .bind(flow_run_id)
                .fetch_optional(&mut **tx)
                .await?;
        row.map(|row| {
            Ok((
                row.try_get("scope_id")?,
                crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(
                    row.try_get::<&str, _>("status")?,
                )?,
            ))
        })
        .transpose()
    }

    /// These IDs were returned by this transaction's waiting_callback → cancelled CAS.
    pub(super) async fn superseded_application_lifecycle_facts(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        ids: &[Uuid],
    ) -> Result<Vec<ApplicationLifecycleFact>> {
        if self.application_lifecycle_observer.is_none() || ids.is_empty() {
            return Ok(vec![]);
        }
        let rows = sqlx::query("select scope_id,application_id,id,run_mode,status,finished_at from flow_runs where id=any($1)")
            .bind(ids).fetch_all(&mut **tx).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ApplicationLifecycleFact {
                    phase: ApplicationLifecyclePhase::Ended,
                    workspace_id: row.try_get("scope_id")?,
                    application_id: row.try_get("application_id")?,
                    flow_run_id: row.try_get("id")?,
                    run_mode: crate::mappers::orchestration_runtime_mapper::parse_flow_run_mode(
                        row.try_get::<&str, _>("run_mode")?,
                    )?,
                    status: crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(
                        row.try_get::<&str, _>("status")?,
                    )?,
                    occurred_at: row.try_get("finished_at")?,
                    workflow_ancestor_ids: self.workflow_ancestor_ids(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "application_lifecycle/_tests/application_lifecycle.rs"]
mod tests;
