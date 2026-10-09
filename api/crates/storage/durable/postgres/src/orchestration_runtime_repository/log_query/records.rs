use super::*;
use crate::runtime_record_repository::{append_filter_clause, to_runtime_model_metadata};
use sqlx::{Postgres, QueryBuilder};

pub(in crate::orchestration_runtime_repository) async fn records(
    store: &PgControlPlaneStore,
    scope_id: Uuid,
    application_ids: &[Uuid],
    query: &ApplicationLogRecordsQuery,
) -> Result<ApplicationLogRecordsPage> {
    let lookahead = log_query_page_lookahead(query.limit)?;
    let limit = query.limit;
    let fields = application_log_query_fields();
    validate_application_log_query_filter(&query.filter)?;
    let field = fields
        .iter()
        .find(|f| f.field == query.sort_field && f.sortable)
        .ok_or_else(|| invalid("log_query_sort"))?;
    let mut applications = application_ids.to_vec();
    applications.sort();
    applications.dedup();
    let identity = application_log_query_fingerprint(scope_id, &applications, query);
    let position: Option<(Option<String>, Uuid)> = decode(query.cursor.as_deref(), &identity)?;
    let mut metadata = to_runtime_model_metadata(
        control_plane_contracts::ports::ModelDefinitionRepository::list_model_definitions(
            store,
            Uuid::nil(),
        )
        .await?
        .into_iter()
        .find(|m| m.code == "application_run_log_tasks" && m.scope_id == domain::SYSTEM_SCOPE_ID)
        .ok_or_else(|| anyhow!("application_run_log_tasks metadata missing"))?,
    )?;
    // UUID columns are declared as display strings in legacy metadata. SQL
    // binding follows their persisted type, without changing field names.
    for field in &fields {
        if let Some(declared) = metadata.fields.iter_mut().find(|f| f.code == field.field) {
            if field.value_type == LogQueryValueType::Uuid {
                declared.field_kind = domain::ModelFieldKind::ManyToOne;
            }
        }
    }
    let mut sql = QueryBuilder::<Postgres>::new(
        "select id,application_id,source_kind,source_id,source_client,source_session_id,source_task_id,native_run_id,log_conversation_id,requested_model_id,reasoning_effort,status,outcome,title,total_tokens,input_tokens,output_tokens,input_cache_hit_tokens,total_cost::text as total_cost,started_at,finished_at,created_at,updated_at from application_run_log_tasks where scope_id=",
    );
    sql.push_bind(scope_id)
        .push(" and application_id=any(")
        .push_bind(applications)
        .push(")");
    append_filter_clause(&mut sql, &metadata, &query.filter)
        .map_err(|_| invalid("log_query_filter"))?;
    let column = format!("\"{}\"", field.field); // closed descriptor above, never arbitrary client SQL
    let direction = if query.descending { " desc" } else { " asc" };
    let comparison = if query.descending { " < " } else { " > " };
    if let Some((date, id)) = position {
        if let Some(date) = date {
            let date = time::OffsetDateTime::parse(&date, &Rfc3339)
                .map_err(|_| invalid("log_query_cursor"))?;
            sql.push(" and (")
                .push(&column)
                .push(comparison)
                .push_bind(date)
                .push(" or (")
                .push(&column)
                .push(" = ")
                .push_bind(date)
                .push(" and id")
                .push(comparison)
                .push_bind(id)
                .push(") or ")
                .push(&column)
                .push(" is null)");
        } else {
            sql.push(" and ")
                .push(&column)
                .push(" is null and id")
                .push(comparison)
                .push_bind(id);
        }
    }
    sql.push(" order by ")
        .push(&column)
        .push(direction)
        .push(" nulls last,id")
        .push(direction)
        .push(" limit ")
        .push_bind(lookahead);
    let mut rows = sql.build().fetch_all(store.pool()).await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if more {
        rows.last()
            .map(|r| {
                encode(
                    (
                        optional_timestamp(r, &field.field)?,
                        r.try_get::<Uuid, _>("id")?,
                    ),
                    &identity,
                )
            })
            .transpose()?
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(ApplicationLogRecordSummary {
                record_id: r.try_get("id")?,
                application_id: r.try_get("application_id")?,
                source_kind: r.try_get("source_kind")?,
                source_id: r.try_get("source_id")?,
                source_client: r.try_get("source_client")?,
                source_session_id: r.try_get("source_session_id")?,
                source_task_id: r.try_get("source_task_id")?,
                native_run_id: r.try_get("native_run_id")?,
                log_conversation_id: r.try_get("log_conversation_id")?,
                requested_model_id: r.try_get("requested_model_id")?,
                reasoning_effort: r.try_get("reasoning_effort")?,
                status: r.try_get("status")?,
                outcome: r.try_get("outcome")?,
                title: r.try_get("title")?,
                total_tokens: r.try_get("total_tokens")?,
                input_tokens: r.try_get("input_tokens")?,
                output_tokens: r.try_get("output_tokens")?,
                input_cache_hit_tokens: r.try_get("input_cache_hit_tokens")?,
                total_cost: r.try_get("total_cost")?,
                cost_breakdown: ApplicationLogCostBreakdown {
                    total_cost: r.try_get("total_cost")?,
                },
                started_at: timestamp(&r, "started_at")?,
                finished_at: optional_timestamp(&r, "finished_at")?,
                created_at: timestamp(&r, "created_at")?,
                updated_at: timestamp(&r, "updated_at")?,
                available_views: vec![
                    ApplicationLogView::Conversation,
                    ApplicationLogView::ClientTrajectory,
                ],
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ApplicationLogRecordsPage { items, next_cursor })
}
