impl PgControlPlaneStore {
    /// Recompute the one task row that owns this run. The task table is the
    /// list unit; the run summary stays one row per run.
    pub(super) async fn refresh_application_run_log_task_for_flow_run(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            "select application_run_log_task_refresh(coalesce(log_task_run_id, flow_run_id)) from application_run_log_summaries where flow_run_id = $1",
        )
        .bind(flow_run_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn get_application_run_log_task(
        &self,
        application_id: Uuid,
        task_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunLogTask>> {
        sqlx::query(
            "select id,application_id,scope_id,member_run_ids,parent_task_run_id,is_root,log_conversation_id,client_thread_id,client_turn_id,subagent_kind,status,outcome,user_input,final_output,final_output_run_id,invocation_count,compaction_count,started_at,finished_at from application_run_log_tasks where application_id = $1 and id = $2",
        )
        .bind(application_id)
        .bind(task_run_id)
        .fetch_optional(self.pool())
        .await?
        .map(map_application_run_log_task)
        .transpose()
    }

    /// Every run belongs to exactly one task; expanding a selection returns the
    /// full membership of each selected run's task in selection order.
    async fn expand_application_run_log_tasks(
        &self,
        application_id: Uuid,
        flow_run_ids: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        let selected = flow_run_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let existing:i64=sqlx::query_scalar("select count(*) from application_run_log_summaries where application_id=$1 and flow_run_id=any($2)")
            .bind(application_id).bind(flow_run_ids).fetch_one(self.pool()).await?;
        if existing as usize != selected.len() {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        }
        sqlx::query_scalar(r#"select member.run_id
            from unnest($2::uuid[]) with ordinality requested(id,position)
            join application_run_log_summaries s on s.flow_run_id=requested.id and s.application_id=$1
            join application_run_log_tasks t on t.id=coalesce(s.log_task_run_id,s.flow_run_id)
            cross join lateral unnest(t.member_run_ids) with ordinality member(run_id,member_position)
            group by member.run_id order by min(requested.position),min(member.member_position)"#)
            .bind(application_id).bind(flow_run_ids).fetch_all(self.pool()).await.map_err(Into::into)
    }

    async fn list_application_run_logs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationRunsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationRunLogSummaryPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;
        let created_after = input.created_after;
        let order_by = Self::application_runs_page_order_by(
            input.sort_by.as_deref(),
            input.sort_order.as_deref(),
        );
        let total = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from application_run_log_tasks where application_id=$1 and is_root and ($2::timestamptz is null or created_at>=$2)",
        )
        .bind(application_id)
        .bind(created_after)
        .fetch_one(self.pool())
        .await?;
        let rows = sqlx::query(&format!(
            "select {} from application_run_log_tasks where application_id=$1 and is_root and ($2::timestamptz is null or created_at>=$2) order by {order_by} limit $3 offset $4",
            APPLICATION_RUN_LOG_TASK_SUMMARY_COLUMNS
        ))
        .bind(application_id)
        .bind(created_after)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;
        let mut items = rows
            .into_iter()
            .map(map_application_run_log_task_summary)
            .collect::<Result<Vec<_>>>()?;
        let flow_run_ids = items.iter().map(|item| item.run.id).collect::<Vec<_>>();
        let count_tokens_results = self
            .list_application_run_count_tokens_results(&flow_run_ids)
            .await?
            .into_iter()
            .map(|result| (result.flow_run_id, result.input_tokens))
            .collect::<std::collections::HashMap<_, _>>();
        for item in &mut items {
            item.count_tokens_input_tokens = count_tokens_results.get(&item.run.id).copied();
        }
        Ok(control_plane_contracts::ports::ApplicationRunLogSummaryPage {
            items,
            total,
            page,
            page_size,
        })
    }

    /// Later calls of the task anchored by `flow_run`, with their own facts so
    /// the anchor trace can nest them as rounds. Empty for non-anchor runs.
    pub(super) async fn list_task_round_traces_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<Vec<domain::ApplicationRunTaskRoundTrace>> {
        let member_ids: Option<Vec<Uuid>> = sqlx::query_scalar(
            "select member_run_ids from application_run_log_tasks where application_id=$1 and id=$2",
        )
        .bind(flow_run.application_id)
        .bind(flow_run.id)
        .fetch_optional(self.pool())
        .await?;
        let mut rounds = Vec::new();
        for member_id in member_ids.unwrap_or_default().into_iter().skip(1) {
            let Some(source_flow_run) =
                fetch_flow_run_for_application(self, flow_run.application_id, member_id).await?
            else {
                continue;
            };
            let call_kind: String = sqlx::query_scalar(
                "select call_kind from application_run_log_summaries where flow_run_id=$1",
            )
            .bind(member_id)
            .fetch_one(self.pool())
            .await?;
            rounds.push(domain::ApplicationRunTaskRoundTrace {
                call_kind,
                node_runs: list_node_runs_for_flow_run(self, member_id).await?,
                callback_tasks: list_callback_tasks_for_flow_run(self, member_id).await?,
                native_messages: self
                    .application_run_native_trace_messages(flow_run.application_id, member_id)
                    .await?,
                source_flow_run,
            });
        }
        Ok(rounds)
    }

    /// Tasks whose client declared `flow_run`'s task as their parent.
    pub(super) async fn list_child_task_traces_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<Vec<domain::ApplicationRunChildTaskTrace>> {
        let children: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            "select id, subagent_kind from application_run_log_tasks where application_id=$1 and parent_task_run_id=$2 order by started_at, id",
        )
        .bind(flow_run.application_id)
        .bind(flow_run.id)
        .fetch_all(self.pool())
        .await?;
        let mut traces = Vec::new();
        for (child_id, subagent_kind) in children {
            let Some(source_flow_run) =
                fetch_flow_run_for_application(self, flow_run.application_id, child_id).await?
            else {
                continue;
            };
            traces.push(domain::ApplicationRunChildTaskTrace {
                subagent_kind,
                node_runs: list_node_runs_for_flow_run(self, child_id).await?,
                callback_tasks: list_callback_tasks_for_flow_run(self, child_id).await?,
                source_flow_run,
            });
        }
        Ok(traces)
    }

    /// Watermark inputs for the task-level parts of the anchor trace. Output
    /// facts are counted through the same projected messages the builder sees,
    /// so duplicate formal events never make the watermark drift.
    pub(super) async fn task_trace_source_counts(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        anchor_output_count: usize,
    ) -> Result<(usize, usize, usize)> {
        let row: Option<(Vec<Uuid>, i64)> = sqlx::query_as(
            r#"select t.member_run_ids,
                (select count(*) from application_run_log_tasks c where c.parent_task_run_id=t.id)::bigint
               from application_run_log_tasks t where t.application_id=$1 and t.id=$2"#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        let Some((members, children)) = row else {
            return Ok((0, 0, 0));
        };
        let mut outputs = anchor_output_count;
        for member in members.iter().skip(1) {
            outputs += self
                .application_run_native_trace_messages(application_id, *member)
                .await?
                .len();
        }
        Ok((
            members.len().saturating_sub(1),
            usize::try_from(children).unwrap_or_default(),
            outputs,
        ))
    }
}

const APPLICATION_RUN_LOG_TASK_SUMMARY_COLUMNS: &str = "id,member_run_ids,parent_task_run_id,log_conversation_id,outcome,user_input,final_output,final_output_run_id,call_kind,invocation_count,compaction_count,run_mode,status,target_node_id,title,'{}'::jsonb as input_payload,external_user,created_by,authorized_account,api_key_id,publication_version_id,external_conversation_id,external_trace_id,compatibility_mode,idempotency_key,total_tokens,input_tokens,output_tokens,input_cache_hit_tokens,input_cache_hit_rate,unique_node_count,tool_callback_count,started_at,finished_at,created_at,updated_at";

fn map_application_run_log_task(row: sqlx::postgres::PgRow) -> Result<domain::ApplicationRunLogTask> {
    let status: String = row.get("status");
    Ok(domain::ApplicationRunLogTask {
        id: row.get("id"),
        application_id: row.get("application_id"),
        scope_id: row.get("scope_id"),
        member_run_ids: row.get("member_run_ids"),
        parent_task_run_id: row.get("parent_task_run_id"),
        is_root: row.get("is_root"),
        log_conversation_id: row.get("log_conversation_id"),
        client_thread_id: row.get("client_thread_id"),
        client_turn_id: row.get("client_turn_id"),
        subagent_kind: row.get("subagent_kind"),
        status: crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(&status)?,
        outcome: row.get("outcome"),
        user_input: row.get("user_input"),
        final_output: row.get("final_output"),
        final_output_run_id: row.get("final_output_run_id"),
        invocation_count: row.get("invocation_count"),
        compaction_count: row.get("compaction_count"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

fn map_application_run_log_task_summary(row: sqlx::postgres::PgRow) -> Result<domain::ApplicationRunLogSummary> {
    let member_run_ids: Vec<Uuid> = row.get("member_run_ids");
    let parent_task_run_id: Option<Uuid> = row.get("parent_task_run_id");
    let outcome: String = row.get("outcome");
    let user_input: Option<String> = row.get("user_input");
    let final_output: Option<String> = row.get("final_output");
    let final_output_run_id: Option<Uuid> = row.get("final_output_run_id");
    let task_id: Uuid = row.get("id");
    let mut summary = map_application_run_log_summary(row)?;
    summary.log_task_run_id = Some(task_id);
    summary.member_run_ids = member_run_ids;
    summary.parent_run_id = parent_task_run_id;
    summary.parent_task_run_id = parent_task_run_id;
    summary.outcome = outcome;
    summary.user_input = user_input;
    summary.final_output = final_output;
    summary.final_output_run_id = final_output_run_id;
    Ok(summary)
}
