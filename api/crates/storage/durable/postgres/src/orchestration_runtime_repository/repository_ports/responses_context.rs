use super::*;
use crate::mappers::orchestration_runtime_mapper::parse_flow_run_status;
use control_plane_contracts::application_public_runtime::PublishedResponsesContext;

/// Read only the public cursor proof and the scalars needed by its consumer.
/// Callback facts outrank mutable flow output; historical rounds must stay historical.
pub(super) async fn find(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    application_id: Uuid,
    api_key_id: Uuid,
    actor_user_id: Uuid,
    response_id: &str,
) -> Result<Vec<PublishedResponsesContext>> {
    let public_run_id = response_id
        .strip_prefix("resp_")
        .and_then(|id| Uuid::parse_str(id).ok());
    let mut rows = sqlx::query(CONTEXT_SQL)
        .bind(workspace_id)
        .bind(application_id)
        .bind(api_key_id)
        .bind(actor_user_id)
        .bind(response_id)
        .bind(public_run_id)
        .fetch_all(store.pool())
        .await?;
    // The ordinary scalar path keeps one query. Only original-output streaming
    // needs a snapshot shared by proof/status selection and the subsequent chunks.
    let mut transaction = None;
    if rows.len() == 1
        && rows[0]
            .get::<Option<bool>, _>("answer_original_present")
            .unwrap_or(false)
    {
        let mut snapshot = store.pool().begin().await?;
        sqlx::query("set transaction isolation level repeatable read read only")
            .execute(&mut *snapshot)
            .await?;
        rows = sqlx::query(CONTEXT_SQL)
            .bind(workspace_id)
            .bind(application_id)
            .bind(api_key_id)
            .bind(actor_user_id)
            .bind(response_id)
            .bind(public_run_id)
            .fetch_all(&mut *snapshot)
            .await?;
        transaction = Some(snapshot);
    }

    let unambiguous = rows.len() == 1;
    let mut contexts = Vec::with_capacity(rows.len());
    for row in rows {
        let history = row
            .get::<Option<Value>, _>("history")
            .map(serde_json::from_value::<domain::orchestration::ResponsesHistoryProof>)
            .transpose()?;
        if let Some(proof) = &history {
            proof.validate().map_err(anyhow::Error::msg)?;
        }
        let flow_run_id = row.get("flow_run_id");
        let answer = if unambiguous
            && row
                .get::<Option<bool>, _>("answer_original_present")
                .unwrap_or(false)
        {
            super::responses_raw_answer::read(
                transaction
                    .as_mut()
                    .expect("raw original is read inside its snapshot"),
                workspace_id,
                application_id,
                api_key_id,
                actor_user_id,
                flow_run_id,
            )
            .await?
        } else {
            row.get::<Option<String>, _>("answer")
                .map(|scalar| serde_json::from_str::<String>(&scalar))
                .transpose()?
        };
        contexts.push(PublishedResponsesContext {
            flow_run_id,
            status: parse_flow_run_status(row.get::<String, _>("status").as_str())?,
            external_user: row.get("external_user"),
            external_conversation_id: row.get("external_conversation_id"),
            answer,
            history,
        });
    }
    if let Some(snapshot) = transaction {
        snapshot.commit().await?;
    }
    Ok(contexts)
}

const CONTEXT_SQL: &str = r#"
        with callback_rounds as (
            select c.flow_run_id,
                case when c.request_payload #>> '{responses_round,response_id}' = $5
                    then c.request_payload #> '{responses_round,history}'
                    else c.request_payload #> '{provider_metadata,native_response,history}' end as proof
            from flow_run_callback_tasks c
            join flow_runs f on f.id = c.flow_run_id
            join applications a on a.id = f.application_id
            where a.workspace_id = $1 and f.application_id = $2
                and f.api_key_id = $3 and f.created_by = $4
                and f.run_mode = 'published_api_run' and c.callback_kind = 'llm_tool_calls'
                and (c.request_payload #>> '{responses_round,response_id}' = $5
                    or c.request_payload #>> '{provider_metadata,native_response,response_id}' = $5)
                and (f.import_job_id is null or exists (
                    select 1 from run_archive_import_jobs j where j.id = f.import_job_id and j.status = 'succeeded'))
        ), public_run_ids as materialized (
            -- A valid public response UUID gets the scoped primary-key path.
            -- Keep it separate from provider aliases: an OR would scan application history.
            select f.id
            from flow_runs f join applications a on a.id = f.application_id
            where not exists (select 1 from callback_rounds)
                and f.id = $6 and a.workspace_id = $1 and f.application_id = $2
                and f.api_key_id = $3 and f.created_by = $4 and f.run_mode = 'published_api_run'
                and (f.import_job_id is null or exists (
                    select 1 from run_archive_import_jobs j where j.id = f.import_job_id and j.status = 'succeeded'))
        ), provider_run_ids as materialized (
            -- Historical provider locator search is a fallback, never part of the PK predicate.
            select f.id
            from flow_runs f join applications a on a.id = f.application_id
            where not exists (select 1 from callback_rounds)
                and not exists (select 1 from public_run_ids)
                and a.workspace_id = $1 and f.application_id = $2
                and f.api_key_id = $3 and f.created_by = $4 and f.run_mode = 'published_api_run'
                and (f.import_job_id is null or exists (
                    select 1 from run_archive_import_jobs j where j.id = f.import_job_id and j.status = 'succeeded'))
                and (f.output_payload #>> '{responses_round,response_id}' = $5
                    or exists (select 1 from node_run_records n where n.flow_run_id = f.id
                        and (n.output_payload ->> 'response_id' = $5
                            or n.output_payload #>> '{provider_metadata,native_response,response_id}' = $5))
                    or exists (select 1 from runtime_spans s where s.flow_run_id = f.id
                        and s.metadata #>> '{native_response,response_id}' = $5))
        ), run_ids as (
            select id from public_run_ids union all select id from provider_run_ids
        ), run_rounds as (
            select f.id as flow_run_id,
                coalesce(
                    case when f.output_payload #>> '{responses_round,response_id}' = $5
                        then f.output_payload #> '{responses_round,history}' end,
                    (select n.output_payload #> '{provider_metadata,native_response,history}'
                        from node_run_records n where n.flow_run_id = f.id
                            and n.output_payload #>> '{provider_metadata,native_response,response_id}' = $5
                        order by n.finished_at desc nulls last, n.id desc limit 1),
                    (select s.metadata #> '{native_response,history}' from runtime_spans s
                        where s.flow_run_id = f.id and s.metadata #>> '{native_response,response_id}' = $5
                        order by s.id desc limit 1)
                ) as proof
            from run_ids join flow_runs f on f.id = run_ids.id
        ), rounds as (
            select * from callback_rounds union all select * from run_rounds
        )
        select f.id as flow_run_id, f.status, f.external_user, f.external_conversation_id,
            case when f.status in ('succeeded', 'incomplete')
                or (f.status in ('failed', 'cancelled', 'waiting_callback', 'waiting_human')
                    and f.output_payload -> '__canonical_answer_presentation' = 'true'::jsonb)
                then case when not (f.raw_json_payloads ? 'output_payload')
                    and jsonb_typeof(coalesce(f.output_payload -> 'answer', f.output_payload -> 'text', f.output_payload -> 'output')) = 'string'
                    then coalesce(f.output_payload -> 'answer', f.output_payload -> 'text', f.output_payload -> 'output')::text end
            end as answer,
            (f.raw_json_payloads ? 'output_payload') and (
                f.status in ('succeeded', 'incomplete')
                or (f.status in ('failed', 'cancelled', 'waiting_callback', 'waiting_human')
                    and f.output_payload -> '__canonical_answer_presentation' = 'true'::jsonb)
            ) as answer_original_present,
            case when rounds.proof is not null and rounds.proof <> 'null'::jsonb
                then jsonb_build_object('version', rounds.proof -> 'version',
                    'item_count', rounds.proof -> 'item_count', 'digest', rounds.proof -> 'digest') end as history
        from rounds join flow_runs f on f.id = rounds.flow_run_id
        order by f.id
    "#;
