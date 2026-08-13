fn parse_resume_claim_kind(value: &str) -> Result<ResumeClaimKind> {
    match value {
        "human" => Ok(ResumeClaimKind::Human),
        "callback" => Ok(ResumeClaimKind::Callback),
        _ => Err(anyhow!("invalid resume claim kind: {value}")),
    }
}

fn parse_resume_claim_status(value: &str) -> Result<ResumeClaimStatus> {
    match value {
        "processing" => Ok(ResumeClaimStatus::Processing),
        "succeeded" => Ok(ResumeClaimStatus::Succeeded),
        "failed" => Ok(ResumeClaimStatus::Failed),
        _ => Err(anyhow!("invalid resume claim status: {value}")),
    }
}

fn map_resume_claim(row: &sqlx::postgres::PgRow) -> Result<ResumeClaimRecord> {
    Ok(ResumeClaimRecord {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        checkpoint_id: row.get("checkpoint_id"),
        callback_task_id: row.get("callback_task_id"),
        kind: parse_resume_claim_kind(row.get::<String, _>("resume_kind").as_str())?,
        status: parse_resume_claim_status(row.get::<String, _>("status").as_str())?,
        request_payload: row.get("request_payload"),
        claim_token: row.get("claim_token"),
        generation: row.get("generation"),
        lease_expires_at: row.get("lease_expires_at"),
        error_payload: row.get("error_payload"),
        completed_at: row.get("completed_at"),
    })
}

const RESUME_CLAIM_COLUMNS: &str = r#"
    id, flow_run_id, checkpoint_id, callback_task_id, resume_kind, status,
    request_payload, claim_token, generation, lease_expires_at, error_payload, completed_at
"#;

impl PgControlPlaneStore {
    async fn acquire_resume_claim(
        &self,
        input: &AcquireResumeClaimInput,
    ) -> Result<AcquireResumeClaimOutput> {
        if (input.kind == ResumeClaimKind::Callback) != input.callback_task_id.is_some() {
            return Err(anyhow!("resume claim kind does not match callback task"));
        }
        let mut tx = self.pool().begin().await?;
        let target_id = input.callback_task_id.unwrap_or(input.checkpoint_id);
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(target_id.to_string())
            .execute(&mut *tx)
            .await?;
        let existing = if let Some(callback_task_id) = input.callback_task_id {
            sqlx::query(&format!(
                "select {RESUME_CLAIM_COLUMNS} from flow_run_resume_claims where callback_task_id = $1 for update"
            ))
            .bind(callback_task_id)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            sqlx::query(&format!(
                "select {RESUME_CLAIM_COLUMNS} from flow_run_resume_claims where checkpoint_id = $1 and resume_kind = 'human' for update"
            ))
            .bind(input.checkpoint_id)
            .fetch_optional(&mut *tx)
            .await?
        };

        if let Some(row) = existing {
            let existing = map_resume_claim(&row)?;
            if existing.flow_run_id != input.flow_run_id
                || existing.checkpoint_id != input.checkpoint_id
                || existing.kind != input.kind
                || existing.request_payload != input.request_payload
            {
                return Err(ControlPlaneError::Conflict("resume_claim_payload_conflict").into());
            }
            let flow_status: String =
                sqlx::query_scalar("select status from flow_runs where id = $1 and application_id = $2 and scope_id = $3")
                    .bind(input.flow_run_id)
                    .bind(input.application_id)
                    .bind(input.scope_id)
                    .fetch_one(&mut *tx)
                    .await?;
            let waiting_status = match input.kind {
                ResumeClaimKind::Human => "waiting_human",
                ResumeClaimKind::Callback => "waiting_callback",
            };
            if existing.status == ResumeClaimStatus::Succeeded || flow_status != waiting_status {
                let row = sqlx::query(&format!(
                    "update flow_run_resume_claims set status = 'succeeded', completed_at = coalesce(completed_at, now()), updated_at = now() where id = $1 returning {RESUME_CLAIM_COLUMNS}"
                ))
                .bind(existing.id)
                .fetch_one(&mut *tx)
                .await?;
                tx.commit().await?;
                return Ok(AcquireResumeClaimOutput {
                    claim: map_resume_claim(&row)?,
                    disposition: ResumeClaimDisposition::Completed,
                });
            }
            if existing.status == ResumeClaimStatus::Processing
                && existing.lease_expires_at > OffsetDateTime::now_utc()
            {
                tx.commit().await?;
                return Ok(AcquireResumeClaimOutput {
                    claim: existing,
                    disposition: ResumeClaimDisposition::InProgress,
                });
            }
            let claim_token = Uuid::now_v7();
            let row = sqlx::query(&format!(
                "update flow_run_resume_claims set status = 'processing', claim_token = $2, generation = generation + 1, lease_expires_at = now() + interval '5 minutes', error_payload = null, completed_at = null, updated_at = now() where id = $1 returning {RESUME_CLAIM_COLUMNS}"
            ))
            .bind(existing.id)
            .bind(claim_token)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(AcquireResumeClaimOutput {
                claim: map_resume_claim(&row)?,
                disposition: ResumeClaimDisposition::Acquired,
            });
        }

        let claim_token = Uuid::now_v7();
        let row = sqlx::query(&format!(
            "insert into flow_run_resume_claims (id, scope_id, application_id, flow_run_id, checkpoint_id, callback_task_id, resume_kind, status, request_payload, claim_token, lease_expires_at) select $1, $2, $3, flow_runs.id, flow_run_checkpoints.id, $6, $7, 'processing', $8, $9, now() + interval '5 minutes' from flow_runs join flow_run_checkpoints on flow_run_checkpoints.id = $5 and flow_run_checkpoints.flow_run_id = flow_runs.id where flow_runs.id = $4 and flow_runs.scope_id = $2 and flow_runs.application_id = $3 and flow_runs.status = $10 returning {RESUME_CLAIM_COLUMNS}"
        ))
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(input.checkpoint_id)
        .bind(input.callback_task_id)
        .bind(input.kind.as_str())
        .bind(&input.request_payload)
        .bind(claim_token)
        .bind(match input.kind {
            ResumeClaimKind::Human => "waiting_human",
            ResumeClaimKind::Callback => "waiting_callback",
        })
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::Conflict("resume_claim_not_waiting"))?;
        tx.commit().await?;
        Ok(AcquireResumeClaimOutput {
            claim: map_resume_claim(&row)?,
            disposition: ResumeClaimDisposition::Acquired,
        })
    }

    async fn finish_resume_claim(&self, input: &FinishResumeClaimInput) -> Result<ResumeClaimRecord> {
        if input.status == ResumeClaimStatus::Processing {
            return Err(anyhow!("resume claim cannot finish as processing"));
        }
        let row = sqlx::query(&format!(
            "update flow_run_resume_claims set status = $3, error_payload = case when status = 'processing' then $4 else error_payload end, completed_at = coalesce(completed_at, $5), updated_at = now() where id = $1 and claim_token = $2 and status in ('processing', $3) returning {RESUME_CLAIM_COLUMNS}"
        ))
        .bind(input.claim_id)
        .bind(input.claim_token)
        .bind(input.status.as_str())
        .bind(&input.error_payload)
        .bind(input.completed_at)
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else {
            return Err(ControlPlaneError::Conflict("resume_claim_not_owned").into());
        };
        map_resume_claim(&row)
    }
}
