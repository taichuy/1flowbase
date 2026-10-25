use super::*;

impl PgControlPlaneStore {
    pub(super) async fn list_application_conversation_runs_page(
        &self,
        application_id: Uuid,
        input: control_plane_contracts::ports::ListApplicationConversationRunsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationConversationRunsPage> {
        let limit = input.limit.clamp(1, 50);
        let (start_rn, end_rn, total) = if let Some(anchor_run_id) = input.before_run_id {
            let Some((anchor_rn, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    Some(anchor_run_id),
                    input.around_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            ((anchor_rn - limit).max(1), anchor_rn - 1, total)
        } else if let Some(anchor_run_id) = input.after_run_id {
            let Some((anchor_rn, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    Some(anchor_run_id),
                    input.around_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            (anchor_rn + 1, (anchor_rn + limit).min(total), total)
        } else {
            // The selected task bounds the entire paging set, including subsequent
            // before/after reads. Without an anchor, open the latest turns.
            let Some((_, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    None,
                    input.around_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            let start_rn = (total - limit + 1).max(1);
            (start_rn, (start_rn + limit - 1).min(total), total)
        };

        if start_rn > end_rn || total == 0 {
            return Ok(empty_application_conversation_runs_page());
        }

        let rows = sqlx::query(
            r#"
            with grouped as (
                select
                    runs.id,
                    runs.status,
                    (
                        array_agg(messages.content order by messages.sequence asc)
                        filter (where messages.role = 'user')
                    )[1] as query,
                    null::text as query_original,
                    null::text as query_raw_source,null::text[] as query_raw_path,null::text as query_raw_mode,
                    null::text as model,
                    (
                        array_agg(messages.content order by messages.sequence asc)
                        filter (where messages.role = 'assistant')
                    )[1] as answer,
                    runs.started_at,
                    runs.finished_at,
                    min(messages.sequence) as order_sequence,
                    coalesce(runs.log_context->>'request_kind','') <> 'prewarm' as is_business_turn
                from application_conversation_messages messages
                join application_conversations conversations
                  on conversations.id = messages.conversation_id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where messages.application_id = $1
                  and conversations.external_conversation_id = $2
                  and (
                      runs.import_job_id is null
                      or exists (
                          select 1
                          from run_archive_import_jobs import_jobs
                          where import_jobs.id = runs.import_job_id
                            and import_jobs.status = 'succeeded'
                      )
                  )
                group by runs.id, runs.status, runs.started_at, runs.finished_at
                union all
                select t.id,t.status,coalesce(recovered.input_text,t.user_input) as query,
                    recovered.input_original as query_original,
                    recovered.input_raw_source as query_raw_source,recovered.input_raw_path as query_raw_path,recovered.input_raw_mode as query_raw_mode,
                    coalesce((select m.model from application_run_conversation_message_items m
                        where m.flow_run_id=coalesce(t.final_output_run_id,t.id) and m.model is not null
                        order by m.is_current desc,m.display_sequence desc limit 1),t.requested_model_id) as model,
                    case when t.outcome='final_answer_observed' then t.final_output end as answer,
                    t.started_at,t.finished_at,
                    (extract(epoch from t.started_at)*1000000)::bigint as order_sequence,
                    coalesce(r.log_context->>'request_kind','') <> 'prewarm' as is_business_turn
                from application_run_log_tasks t
                join flow_runs r on r.id=t.id
                left join lateral application_run_log_task_user_input_source(t.id) recovered on true
                where t.application_id=$1 and t.is_root
                  and t.log_conversation_id=case when $2 ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then $2::uuid end
            ),
            ordered as (
                select
                    id,
                    status,
                    query,
                    query_original,query_raw_source,query_raw_path,query_raw_mode,
                    model,
                    answer,
                    started_at,
                    finished_at,
                    row_number() over (order by order_sequence asc, id asc) as rn
                from grouped
                where is_business_turn
                  and ($5::uuid is null or (order_sequence, id) <= (
                      select anchor.order_sequence, anchor.id from grouped anchor
                      where anchor.id = coalesce(
                          (select s.log_task_run_id from application_run_log_summaries s
                           where s.application_id=$1 and s.flow_run_id=$5), $5)
                  ))
            )
            select
                id,
                status,
                query,
                query_original,query_raw_source,query_raw_path,query_raw_mode,
                model,
                answer,
                started_at,
                finished_at
            from ordered
            where rn between $3 and $4
            order by rn asc
            "#,
        )
        .bind(application_id)
        .bind(&input.external_conversation_id)
        .bind(start_rn)
        .bind(end_rn)
        .bind(input.around_run_id)
        .fetch_all(self.pool())
        .await?;

        let items = rows
            .into_iter()
            .map(|row| {
                let query = application_run_log_current_input_text(&row, "query")?;
                let mut summary = map_application_conversation_run_summary(row)?;
                summary.query = query;
                Ok(summary)
            })
            .collect::<Result<Vec<_>>>()?;
        let before_cursor = items.first().map(|run| run.id);
        let after_cursor = items.last().map(|run| run.id);

        Ok(
            control_plane_contracts::ports::ApplicationConversationRunsPage {
                items,
                has_before: start_rn > 1,
                has_after: end_rn < total,
                before_cursor,
                after_cursor,
            },
        )
    }

    pub(super) async fn application_conversation_run_position(
        &self,
        application_id: Uuid,
        external_conversation_id: &str,
        flow_run_id: Option<Uuid>,
        around_run_id: Option<Uuid>,
    ) -> Result<Option<(i64, i64)>> {
        let row = sqlx::query(
            r#"
            with grouped as (
                select
                    runs.id,
                    min(messages.sequence) as order_sequence,
                    coalesce(runs.log_context->>'request_kind','') <> 'prewarm' as is_business_turn
                from application_conversation_messages messages
                join application_conversations conversations
                  on conversations.id = messages.conversation_id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where messages.application_id = $1
                  and conversations.external_conversation_id = $2
                  and (
                      runs.import_job_id is null
                      or exists (
                          select 1
                          from run_archive_import_jobs import_jobs
                          where import_jobs.id = runs.import_job_id
                            and import_jobs.status = 'succeeded'
                      )
                  )
                group by runs.id
                union all
                select t.id,(extract(epoch from t.started_at)*1000000)::bigint as order_sequence,
                    coalesce(r.log_context->>'request_kind','') <> 'prewarm' as is_business_turn
                from application_run_log_tasks t
                join flow_runs r on r.id=t.id
                where t.application_id=$1 and t.is_root
                  and t.log_conversation_id=case when $2 ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' then $2::uuid end
            ),
            ordered as (
                select
                    id,
                    row_number() over (order by order_sequence asc, id asc) as rn,
                    count(*) over () as total
                from grouped
                where is_business_turn
                  and ($4::uuid is null or (order_sequence, id) <= (
                      select anchor.order_sequence, anchor.id from grouped anchor
                      where anchor.id = coalesce(
                          (select s.log_task_run_id from application_run_log_summaries s
                           where s.application_id=$1 and s.flow_run_id=$4), $4)
                  ))
            )
            select rn, total
            from ordered
            where $3::uuid is null or id = $3
            order by rn desc
            limit 1
            "#,
        )
        .bind(application_id)
        .bind(external_conversation_id)
        .bind(flow_run_id)
        .bind(around_run_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| (row.get("rn"), row.get("total"))))
    }

    pub(super) fn application_runs_page_order_by(
        sort_by: Option<&str>,
        sort_order: Option<&str>,
    ) -> String {
        let sort_by = sort_by.unwrap_or("created_at").to_ascii_lowercase();
        let sort_order = sort_order.unwrap_or("desc").to_ascii_lowercase();
        let field = match sort_by.as_str() {
            "started_at" => "started_at",
            "finished_at" => "finished_at",
            "updated_at" => "updated_at",
            "created_at" => "created_at",
            _ => "created_at",
        };
        let direction = match sort_order.as_str() {
            "asc" => "asc",
            _ => "desc",
        };

        format!("{field} {direction}, id {direction}")
    }
}
