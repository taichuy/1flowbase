use super::*;
use control_plane_contracts::ports::{
    CommitContainerTransitionInput, ContainerCommitAdmission, ContainerNodeFact,
};

impl PgControlPlaneStore {
    pub(super) async fn container_native_recovery_started(
        &self,
        flow_run_id: Uuid,
    ) -> Result<bool> {
        Ok(sqlx::query_scalar(
            "select native_recovery_started from flow_container_checkpoints where flow_run_id=$1",
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?
        .unwrap_or(false))
    }

    pub(super) async fn load_container_checkpoint(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<Value>> {
        let row = sqlx::query("select runtime_original_json(c.checkpoint,c.raw_json_payloads,'checkpoint') as checkpoint,exists(select 1 from node_runs n where n.flow_run_id=c.flow_run_id and n.started_at>c.updated_at and not exists(select 1 from flow_container_executions e where e.node_run_id=n.id)) as unscoped_progress from flow_container_checkpoints c where c.flow_run_id=$1")
            .bind(flow_run_id).fetch_optional(self.pool()).await?;
        row.map(|row| {
            if row.get::<bool, _>("unscoped_progress") {
                return Err(
                    ControlPlaneError::Conflict("container_root_recovery_uncertain").into(),
                );
            }
            Ok(row.get("checkpoint"))
        })
        .transpose()
    }

    pub(super) async fn get_container_node_run_id(
        &self,
        flow_run_id: Uuid,
        execution_id: Uuid,
    ) -> Result<Option<Uuid>> {
        Ok(sqlx::query_scalar::<_, Option<Uuid>>("select node_run_id from flow_container_executions where flow_run_id=$1 and execution_id=$2")
            .bind(flow_run_id).bind(execution_id).fetch_optional(self.pool()).await?.flatten())
    }

    pub(super) async fn get_container_node_run(
        &self,
        flow_run_id: Uuid,
        execution_id: Uuid,
    ) -> Result<Option<domain::NodeRunRecord>> {
        let row = sqlx::query("select n.id,n.flow_run_id,n.node_id,n.node_type,n.node_alias,n.status,n.started_at,n.finished_at,(select jsonb_build_object('execution_id',e.execution_id,'node_id',e.node_id,'parent_execution_id',e.parent_execution_id,'round_path',e.round_path) from flow_container_executions e where e.node_run_id=n.id) as execution_identity,runtime_original_json(n.input_payload,n.raw_json_payloads,'input_payload') as input_payload,runtime_original_json(n.output_payload,n.raw_json_payloads,'output_payload') as output_payload,runtime_original_json(n.error_payload,n.raw_json_payloads,'error_payload') as error_payload,runtime_original_json(n.metrics_payload,n.raw_json_payloads,'metrics_payload') as metrics_payload,runtime_original_json(n.debug_payload,n.raw_json_payloads,'debug_payload') as debug_payload from node_run_records n join flow_container_executions e on e.node_run_id=n.id where e.flow_run_id=$1 and e.execution_id=$2")
            .bind(flow_run_id).bind(execution_id).fetch_optional(self.pool()).await?;
        row.map(map_node_run_record).transpose()
    }

    pub(super) async fn commit_container_transition(
        &self,
        input: &CommitContainerTransitionInput,
    ) -> Result<Option<domain::NodeRunRecord>> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, input.flow_run_id).await?;
        let scope_id: Option<Uuid> = sqlx::query_scalar("select scope_id from flow_runs where id=$1 and status not in ('succeeded','incomplete','failed','cancelled') for update")
            .bind(input.flow_run_id).fetch_optional(&mut *tx).await?;
        let scope_id = scope_id.ok_or(ControlPlaneError::Conflict("flow_run_terminal"))?;
        // Claim acquisition locks the same parent run before touching the claim.
        // This fence and all following mutations therefore share one commit point.
        if let Some(owner) = &input.resume_claim_owner {
            let owned: Option<Uuid> = sqlx::query_scalar("select id from flow_run_resume_claims where id=$1 and flow_run_id=$2 and claim_token=$3 and generation=$4 and status='processing' and lease_expires_at > clock_timestamp() for update")
                .bind(owner.claim_id).bind(input.flow_run_id).bind(owner.claim_token).bind(owner.expected_generation)
                .fetch_optional(&mut *tx).await?;
            if owned.is_none() {
                return Err(ControlPlaneError::Conflict("resume_claim_not_owned").into());
            }
        } else {
            let transferred: bool = sqlx::query_scalar(
                "select exists(select 1 from flow_run_resume_claims where flow_run_id=$1)",
            )
            .bind(input.flow_run_id)
            .fetch_one(&mut *tx)
            .await?;
            if transferred {
                return Err(ControlPlaneError::Conflict("resume_claim_not_owned").into());
            }
        }
        let identity = &input.execution_identity;
        let round_path = serde_json::to_value(&identity.round_path)?;
        let inserted = sqlx::query("insert into flow_container_executions(flow_run_id,execution_id,node_id,parent_execution_id,round_path) values($1,$2,$3,$4,$5) on conflict(flow_run_id,execution_id) do update set node_id=excluded.node_id where flow_container_executions.node_id=excluded.node_id and flow_container_executions.parent_execution_id is not distinct from excluded.parent_execution_id and flow_container_executions.round_path=excluded.round_path")
            .bind(input.flow_run_id).bind(identity.execution_id).bind(&identity.node_id).bind(identity.parent_execution_id).bind(round_path)
            .execute(&mut *tx).await?;
        if inserted.rows_affected() != 1 {
            return Err(
                ControlPlaneError::Conflict("container_execution_identity_conflict").into(),
            );
        }
        if let Some(node) = &input.node {
            match node {
                ContainerNodeFact::Started {
                    node_type,
                    node_alias,
                    input_payload,
                } => {
                    let node_run_id = Uuid::now_v7();
                    let linked = sqlx::query("update flow_container_executions set node_run_id=$3 where flow_run_id=$1 and execution_id=$2 and node_run_id is null")
                        .bind(input.flow_run_id).bind(identity.execution_id).bind(node_run_id);
                    // Insert before assigning the foreign key; a repeated start rolls
                    // back the whole transaction, including this candidate record.
                    sqlx::query("insert into node_run_records(id,scope_id,flow_run_id,node_id,node_type,node_alias,status,input_payload,debug_payload,started_at,raw_json_payloads) values($1,$2,$3,$4,$5,$6,'running',($7::jsonb->0),$8,now(),jsonb_strip_nulls(jsonb_build_object('input_payload',($7::jsonb->1))))")
                        .bind(node_run_id).bind(scope_id).bind(input.flow_run_id).bind(&identity.node_id)
                        .bind(node_type).bind(node_alias).bind(lossless_json_parameter(input_payload))
                        .bind(json!({})).execute(&mut *tx).await?;
                    if linked.execute(&mut *tx).await?.rows_affected() != 1 {
                        return Err(
                            ControlPlaneError::Conflict("container_node_already_started").into(),
                        );
                    }
                }
                ContainerNodeFact::Updated {
                    status,
                    output_payload,
                    error_payload,
                    metrics_payload,
                    debug_payload,
                    terminal,
                } => {
                    let mut debug = debug_payload.clone();
                    if !debug.is_object() {
                        debug = json!({});
                    }
                    if let Some(debug) = debug.as_object_mut() {
                        debug.remove("provider_events");
                    }
                    let changed = sqlx::query("update node_run_records n set status=$3,output_payload=($4::jsonb->0),error_payload=($5::jsonb->0),metrics_payload=($6::jsonb->0),debug_payload=($7::jsonb->0),finished_at=case when $8 then now() else null end,raw_json_payloads=(n.raw_json_payloads-'output_payload'-'error_payload'-'metrics_payload'-'debug_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload',($4::jsonb->1),'error_payload',($5::jsonb->1),'metrics_payload',($6::jsonb->1),'debug_payload',($7::jsonb->1))) from flow_container_executions e where e.flow_run_id=$1 and e.execution_id=$2 and n.id=e.node_run_id and n.finished_at is null and n.status not in ('succeeded','failed','cancelled')")
                        .bind(input.flow_run_id).bind(identity.execution_id).bind(status.as_str())
                        .bind(lossless_json_parameter(output_payload)).bind(lossless_json_parameter(error_payload))
                        .bind(lossless_json_parameter(metrics_payload)).bind(lossless_json_parameter(&debug)).bind(terminal)
                        .execute(&mut *tx).await?;
                    if changed.rows_affected() != 1 {
                        return Err(ControlPlaneError::Conflict(
                            "container_node_terminal_or_missing",
                        )
                        .into());
                    }
                }
            }
        }
        for round in &input.rounds {
            let round_index = i64::try_from(round.round_index)?;
            let changed = sqlx::query("insert into flow_container_rounds(flow_run_id,execution_id,container_execution_id,round_index,deadline_unix_ms,status,error_payload,output_payload,variables,raw_json_payloads,finished_at) values($1,$2,$3,$4,$5,$6,($7::jsonb->0),($8::jsonb->0),($9::jsonb->0),jsonb_strip_nulls(jsonb_build_object('error_payload',($7::jsonb->1),'output_payload',($8::jsonb->1),'variables',($9::jsonb->1))),case when $6 in ('succeeded','failed','cancelled') then now() else null end) on conflict(flow_run_id,execution_id) do update set status=excluded.status,finished_at=coalesce(flow_container_rounds.finished_at,excluded.finished_at),error_payload=excluded.error_payload,output_payload=excluded.output_payload,variables=excluded.variables,raw_json_payloads=excluded.raw_json_payloads,deadline_unix_ms=excluded.deadline_unix_ms where flow_container_rounds.container_execution_id=excluded.container_execution_id and flow_container_rounds.round_index=excluded.round_index and (flow_container_rounds.deadline_unix_ms is null or (excluded.deadline_unix_ms is not null and flow_container_rounds.deadline_unix_ms>=excluded.deadline_unix_ms)) and (flow_container_rounds.status in ('running','waiting') or (flow_container_rounds.status=excluded.status and flow_container_rounds.error_payload is not distinct from excluded.error_payload and flow_container_rounds.output_payload is not distinct from excluded.output_payload and flow_container_rounds.variables is not distinct from excluded.variables))")
                .bind(input.flow_run_id).bind(round.execution_id).bind(round.container_execution_id).bind(round_index)
                .bind(round.deadline_unix_ms).bind(&round.status).bind(lossless_json_parameter(&round.error_payload)).bind(lossless_json_parameter(&round.output_payload)).bind(lossless_json_parameter(&round.variables)).execute(&mut *tx).await?;
            if changed.rows_affected() != 1 {
                return Err(ControlPlaneError::Conflict(
                    "container_round_terminal_or_identity_conflict",
                )
                .into());
            }
            if matches!(round.status.as_str(), "cancelled" | "failed") {
                // Round closure fences unfinished descendants as part of the same
                // commit. Previously terminal facts cannot be rewritten by a stop.
                sqlx::query("update node_run_records n set status=$3,error_payload=($4::jsonb->0),finished_at=now(),raw_json_payloads=(n.raw_json_payloads-'error_payload') || jsonb_strip_nulls(jsonb_build_object('error_payload',($4::jsonb->1))) from flow_container_executions e where e.flow_run_id=$1 and n.id=e.node_run_id and e.round_path @> $2 and n.finished_at is null and n.status in ('running','waiting_human','waiting_callback','retrying')")
                    .bind(input.flow_run_id)
                    .bind(json!([{"container_execution_id":round.container_execution_id,"round_index":round.round_index}]))
                    .bind(&round.status).bind(lossless_json_parameter(&round.error_payload)).execute(&mut *tx).await?;
            }
        }
        if let Some(failure) = &input.control_failure {
            let changed = sqlx::query("insert into flow_container_control_failures(flow_run_id,execution_id,after_round_index,error_payload,raw_json_payloads) values($1,$2,$3,($4::jsonb->0),jsonb_strip_nulls(jsonb_build_object('error_payload',($4::jsonb->1)))) on conflict(flow_run_id,execution_id,after_round_index) do update set error_payload=flow_container_control_failures.error_payload where flow_container_control_failures.error_payload=excluded.error_payload and flow_container_control_failures.raw_json_payloads=excluded.raw_json_payloads")
                .bind(input.flow_run_id).bind(identity.execution_id).bind(i64::try_from(failure.after_round_index)?)
                .bind(lossless_json_parameter(&failure.error_payload)).execute(&mut *tx).await?;
            if changed.rows_affected() != 1 {
                return Err(
                    ControlPlaneError::Conflict("container_control_failure_conflict").into(),
                );
            }
        }
        if let Some(observation) = &input.observation {
            sqlx::query("insert into flow_container_observations(id,flow_run_id,execution_id,observation_key,trace_payload,raw_json_payloads) values($1,$2,$3,$4,($5::jsonb->0),jsonb_strip_nulls(jsonb_build_object('trace_payload',($5::jsonb->1)))) on conflict(flow_run_id,execution_id,observation_key) do nothing")
                .bind(Uuid::now_v7()).bind(input.flow_run_id).bind(identity.execution_id).bind(&observation.observation_key)
                .bind(lossless_json_parameter(&observation.trace_payload)).execute(&mut *tx).await?;
        }
        if let Some(owner) = &input.resume_claim_owner {
            let live: bool = sqlx::query_scalar(
                "select lease_expires_at>clock_timestamp() from flow_run_resume_claims where id=$1",
            )
            .bind(owner.claim_id)
            .fetch_one(&mut *tx)
            .await?;
            if !live {
                return Err(ControlPlaneError::Conflict("resume_claim_not_owned").into());
            }
        }
        sqlx::query("insert into flow_container_checkpoints(flow_run_id,checkpoint,raw_json_payloads) values($1,($2::jsonb->0),jsonb_strip_nulls(jsonb_build_object('checkpoint',($2::jsonb->1)))) on conflict(flow_run_id) do update set checkpoint=excluded.checkpoint,raw_json_payloads=excluded.raw_json_payloads,revision=flow_container_checkpoints.revision+1,updated_at=now(),native_recovery_started=flow_container_checkpoints.native_recovery_started or coalesce(flow_container_checkpoints.native_recovery_execution_id=$3,false)")
            .bind(input.flow_run_id).bind(lossless_json_parameter(&input.checkpoint)).bind(identity.execution_id).execute(&mut *tx).await?;
        let row = sqlx::query("select n.id,n.flow_run_id,n.node_id,n.node_type,n.node_alias,n.status,n.started_at,n.finished_at,(select jsonb_build_object('execution_id',e.execution_id,'node_id',e.node_id,'parent_execution_id',e.parent_execution_id,'round_path',e.round_path) from flow_container_executions e where e.node_run_id=n.id) as execution_identity,runtime_original_json(n.input_payload,n.raw_json_payloads,'input_payload') as input_payload,runtime_original_json(n.output_payload,n.raw_json_payloads,'output_payload') as output_payload,runtime_original_json(n.error_payload,n.raw_json_payloads,'error_payload') as error_payload,runtime_original_json(n.metrics_payload,n.raw_json_payloads,'metrics_payload') as metrics_payload,runtime_original_json(n.debug_payload,n.raw_json_payloads,'debug_payload') as debug_payload from node_run_records n join flow_container_executions e on e.node_run_id=n.id where e.flow_run_id=$1 and e.execution_id=$2")
            .bind(input.flow_run_id).bind(identity.execution_id).fetch_optional(&mut *tx).await?;
        // Serialized admission point: use the DB wall clock after ALL writes,
        // including the recovery head, while the flow/claim locks remain held.
        // Closure and observation reconciliation remain valid after expiry.
        if let ContainerCommitAdmission::AdmitWork {
            effective_deadline: Some(deadline),
        } = input.admission
        {
            let now_ms: i64 = sqlx::query_scalar(
                "select floor(extract(epoch from clock_timestamp())*1000)::bigint",
            )
            .fetch_one(&mut *tx)
            .await?;
            if now_ms >= deadline.deadline_unix_ms {
                return Err(ControlPlaneError::Conflict("container_deadline_expired").into());
            }
        }
        tx.commit().await?;
        row.map(map_node_run_record).transpose()
    }
}

impl PgControlPlaneStore {
    /// Native recovery creates a new run, but preserves logical scope identity.
    /// Copy only identities still referenced by the current recovery head. SQL
    /// performs the copy without loading historical node payloads into the host.
    pub(super) async fn fork_container_checkpoint(
        &self,
        source_flow_run_id: Uuid,
        target_flow_run_id: Uuid,
        execution_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, target_flow_run_id).await?;
        let allowed: Option<Uuid> = sqlx::query_scalar("select target.id from flow_runs target join flow_runs source on source.id=$1 and source.application_id=target.application_id and source.scope_id=target.scope_id where target.id=$2 and target.status='running' and source.status='failed' and target.input_payload #>> '{sys,native_inference_recovery,failed_flow_run_id}'=$1::text for update of target")
            .bind(source_flow_run_id).bind(target_flow_run_id).fetch_optional(&mut *tx).await?;
        if allowed.is_none() {
            return Err(ControlPlaneError::Conflict("container_recovery_grant_mismatch").into());
        }
        let copied = sqlx::query("insert into flow_container_checkpoints(flow_run_id,checkpoint,raw_json_payloads,native_recovery_execution_id) select $2,checkpoint,raw_json_payloads,$3 from flow_container_checkpoints where flow_run_id=$1 on conflict do nothing")
            .bind(source_flow_run_id).bind(target_flow_run_id).bind(execution_id).execute(&mut *tx).await?;
        if copied.rows_affected() == 0 {
            let exists: bool = sqlx::query_scalar(
                "select exists(select 1 from flow_container_checkpoints where flow_run_id=$1)",
            )
            .bind(target_flow_run_id)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                return Err(ControlPlaneError::Conflict("container_checkpoint_missing").into());
            }
            tx.commit().await?;
            return Ok(());
        }
        // Read only scalar coordinates of the active scope tree; payloads stay
        // in SQL and are copied directly to each successor receipt.
        let scopes = sqlx::query("select e.execution_id,e.node_run_id,(e.execution_id=$2 or e.execution_id in (select (value->>'container_execution_id')::uuid from flow_container_executions waiter,lateral jsonb_array_elements(waiter.round_path) value where waiter.flow_run_id=$1 and waiter.execution_id=$2)) as reopen from flow_container_executions e where e.flow_run_id=$1 and e.execution_id in (select (value #>> '{}')::uuid from flow_container_checkpoints c, lateral jsonb_path_query(c.checkpoint,'$.frames.*.identity.execution_id') value where c.flow_run_id=$1 union select (value #>> '{}')::uuid from flow_container_checkpoints c, lateral jsonb_path_query(c.checkpoint,'$.frames.*.rounds.*.nodes.*.identity.execution_id') value where c.flow_run_id=$1 union select (value #>> '{}')::uuid from flow_container_checkpoints c, lateral jsonb_path_query(c.checkpoint,'$.root_nodes.*.identity.execution_id') value where c.flow_run_id=$1)")
            .bind(source_flow_run_id).bind(execution_id).fetch_all(&mut *tx).await?;
        for scope in scopes {
            let logical_id: Uuid = scope.get("execution_id");
            let source_node: Option<Uuid> = scope.get("node_run_id");
            let reopen: bool = scope.get("reopen");
            let target_node = source_node.map(|_| Uuid::now_v7());
            if let (Some(source_node), Some(target_node)) = (source_node, target_node) {
                sqlx::query("insert into node_run_records(id,scope_id,flow_run_id,node_id,node_type,node_alias,status,input_payload,output_payload,error_payload,metrics_payload,debug_payload,started_at,finished_at,raw_json_payloads) select $2,n.scope_id,$3,n.node_id,n.node_type,n.node_alias,case when $4 then 'running' else n.status end,n.input_payload,n.output_payload,case when $4 then null else n.error_payload end,n.metrics_payload,n.debug_payload,n.started_at,case when $4 then null else n.finished_at end,case when $4 then n.raw_json_payloads-'error_payload' else n.raw_json_payloads end from node_run_records n where n.id=$1")
                    .bind(source_node).bind(target_node).bind(target_flow_run_id).bind(reopen).execute(&mut *tx).await?;
            }
            sqlx::query("insert into flow_container_executions(flow_run_id,execution_id,node_id,parent_execution_id,round_path,node_run_id) select $2,execution_id,node_id,parent_execution_id,round_path,$4 from flow_container_executions where flow_run_id=$1 and execution_id=$3")
                .bind(source_flow_run_id).bind(target_flow_run_id).bind(logical_id).bind(target_node).execute(&mut *tx).await?;
        }
        sqlx::query("insert into flow_container_rounds(flow_run_id,execution_id,container_execution_id,round_index,deadline_unix_ms,status,error_payload,output_payload,variables,raw_json_payloads,started_at,finished_at) select $2,r.execution_id,r.container_execution_id,r.round_index,r.deadline_unix_ms,case when exists(select 1 from flow_container_executions waiter where waiter.flow_run_id=$1 and waiter.execution_id=$3 and waiter.round_path @> jsonb_build_array(jsonb_build_object('container_execution_id',r.container_execution_id,'round_index',r.round_index))) then 'running' else r.status end,r.error_payload,r.output_payload,r.variables,r.raw_json_payloads,r.started_at,case when exists(select 1 from flow_container_executions waiter where waiter.flow_run_id=$1 and waiter.execution_id=$3 and waiter.round_path @> jsonb_build_array(jsonb_build_object('container_execution_id',r.container_execution_id,'round_index',r.round_index))) then null else r.finished_at end from flow_container_rounds r where r.flow_run_id=$1 and exists(select 1 from flow_container_executions e where e.flow_run_id=$2 and e.execution_id=r.container_execution_id)")
            .bind(source_flow_run_id).bind(target_flow_run_id).bind(execution_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
