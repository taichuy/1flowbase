use super::*;
use control_plane_contracts::ports::{
    ApplicationRunTraceNodeContentProjectionInput, ApplicationRunTraceNodeProjectionInput,
};
use std::collections::HashSet;

// Transport batches, not a limit on accepted trace size. The final row can take
// a batch over its byte target. Keep encoded arguments and SQL below PG's bind
// limit without materializing all shared content at once.
const BATCH_BYTES: usize = 64 * 1024;
const PG_BIND_LIMIT: usize = 65_535;

pub(super) async fn write_projection(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    scope_id: Uuid,
    input: &ReplaceApplicationRunTraceProjectionInput,
) -> Result<()> {
    let node_ids: Vec<_> = input.nodes.iter().map(|node| node.trace_node_id).collect();
    let locators: Vec<_> = input
        .nodes
        .iter()
        .map(|node| node.stable_locator.as_str())
        .collect();
    let content_ids: Vec<_> = input
        .contents
        .iter()
        .map(|content| content.trace_node_id)
        .collect();
    // Full replacement rejected duplicate keys even when they straddled SQL
    // batches. Do not turn an invalid projection into last-writer-wins upserts.
    let node_keys: HashSet<_> = node_ids.iter().collect();
    anyhow::ensure!(node_keys.len() == node_ids.len(), "duplicate trace node id");
    anyhow::ensure!(
        locators.iter().collect::<HashSet<_>>().len() == locators.len(),
        "duplicate trace stable locator"
    );
    anyhow::ensure!(
        content_ids.iter().collect::<HashSet<_>>().len() == content_ids.len(),
        "duplicate trace content id"
    );
    anyhow::ensure!(
        content_ids.iter().all(|id| node_keys.contains(id)),
        "trace content must belong to a projected node"
    );
    drop(node_keys);

    // ON DELETE CASCADE acts at statement time even though the parent FK is
    // deferred. Detach children before deleting obsolete identities/locators;
    // every surviving parent link is restored by the complete node upsert.
    sqlx::query(
        r#"
        with removed as (
            select old.trace_node_id
            from application_run_trace_nodes old
            where old.flow_run_id = $1 and not exists (
                select 1 from unnest($2::uuid[], $3::text[]) desired(id, locator)
                where desired.id = old.trace_node_id and desired.locator = old.stable_locator
            )
        )
        update application_run_trace_nodes child set parent_trace_node_id = null
        from removed
        where child.flow_run_id = $1 and child.parent_trace_node_id = removed.trace_node_id
        "#,
    )
    .bind(input.flow_run_id)
    .bind(&node_ids)
    .bind(&locators)
    .execute(&mut **tx)
    .await?;

    // A surviving node can lose its content without itself being removed.
    sqlx::query("delete from application_run_trace_node_contents where flow_run_id = $1 and not (trace_node_id = any($2))")
        .bind(input.flow_run_id).bind(&content_ids).execute(&mut **tx).await?;
    sqlx::query(
        r#"
        delete from application_run_trace_nodes old
        where old.flow_run_id = $1 and not exists (
            select 1 from unnest($2::uuid[], $3::text[]) desired(id, locator)
            where desired.id = old.trace_node_id and desired.locator = old.stable_locator
        )
        "#,
    )
    .bind(input.flow_run_id)
    .bind(&node_ids)
    .bind(&locators)
    .execute(&mut **tx)
    .await?;

    write_nodes(tx, scope_id, input).await?;
    write_contents(tx, scope_id, input).await
}

struct EncodedNode<'a> {
    node: &'a ApplicationRunTraceNodeProjectionInput,
    metrics: String,
    content_ref: Option<String>,
}

async fn write_nodes(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    scope_id: Uuid,
    input: &ReplaceApplicationRunTraceProjectionInput,
) -> Result<()> {
    let mut nodes = input.nodes.iter().peekable();
    while nodes.peek().is_some() {
        let mut batch = Vec::new();
        let mut bytes = 0usize;
        for node in nodes.by_ref() {
            let metrics = serde_json::to_string(&lossless_json_value(&node.metrics_payload))?;
            let content_ref = lossless_text_parameter(&node.content_ref)
                .map(|packet| serde_json::to_string(&packet))
                .transpose()?;
            bytes = bytes
                .saturating_add(metrics.len())
                .saturating_add(content_ref.as_ref().map_or(0, String::len))
                .saturating_add(
                    node.stable_locator.len()
                        + node.node_kind.len()
                        + node.order_key.len()
                        + node.node_alias.len()
                        + node.status.len()
                        + input.source_watermark.len(),
                )
                .saturating_add(
                    [
                        &node.owner_kind,
                        &node.owner_id,
                        &node.node_id,
                        &node.node_type,
                        &node.node_mode,
                        &node.parent_tool_call_id,
                        &node.trace_relation_kind,
                    ]
                    .iter()
                    .map(|value| value.as_ref().map_or(0, String::len))
                    .sum::<usize>()
                        + 256,
                );
            batch.push(EncodedNode {
                node,
                metrics,
                content_ref,
            });
            if bytes >= BATCH_BYTES || batch.len() == PG_BIND_LIMIT / 30 {
                break;
            }
        }
        let mut query = QueryBuilder::<Postgres>::new(include_str!("nodes_head.sql"));
        query.push_values(&batch, |mut row, encoded| {
            let node = encoded.node;
            row.push_bind(Uuid::now_v7())
                .push_bind(scope_id)
                .push_bind(node.trace_node_id)
                .push_bind(input.flow_run_id)
                .push_bind(node.parent_trace_node_id)
                .push_bind(&node.stable_locator)
                .push_bind(&node.node_kind)
                .push_bind(&node.owner_kind)
                .push_bind(&node.owner_id)
                .push_bind(&node.order_key)
                .push_bind(&node.node_id)
                .push_bind(&node.node_type)
                .push_bind(&node.node_mode)
                .push_bind(&node.node_alias)
                .push_bind(&node.status)
                .push_bind(node.started_at)
                .push_bind(node.finished_at)
                .push_bind(node.duration_ms)
                .push_bind(&encoded.metrics)
                .push_bind(node.has_children)
                .push_bind(node.child_count)
                .push_bind(node.has_content)
                .push_bind(&encoded.content_ref)
                .push_bind(node.source_flow_run_id)
                .push_bind(node.source_trace_node_id)
                .push_bind(node.parent_callback_task_id)
                .push_bind(&node.parent_tool_call_id)
                .push_bind(&node.trace_relation_kind)
                .push_bind(input.projection_version)
                .push_bind(&input.source_watermark);
        });
        query.push(include_str!("nodes_tail.sql"));
        let written = query.build().execute(&mut **tx).await?.rows_affected();
        // Nodes deliberately have no DISTINCT predicate: row-local watermark
        // and public materialization timestamps advance for the entire snapshot.
        // The conflict WHERE must never silently accept another run's identity.
        anyhow::ensure!(
            written == batch.len() as u64,
            "trace node belongs to another flow run"
        );
    }
    Ok(())
}

struct EncodedContent<'a> {
    content: &'a ApplicationRunTraceNodeContentProjectionInput,
    payload: String,
    source_refs: String,
}

async fn write_contents(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    scope_id: Uuid,
    input: &ReplaceApplicationRunTraceProjectionInput,
) -> Result<()> {
    let mut contents = input.contents.iter().peekable();
    while contents.peek().is_some() {
        let mut batch = Vec::new();
        let mut bytes = 0usize;
        for content in contents.by_ref() {
            let payload = {
                let payload = content.payload.materialize_for_persistence();
                serde_json::to_string(&lossless_json_value(payload.as_ref()))?
            };
            let source_refs = serde_json::to_string(&lossless_json_value(&content.source_refs))?;
            bytes = bytes
                .saturating_add(payload.len())
                .saturating_add(source_refs.len())
                .saturating_add(content.content_kind.len() + 64);
            batch.push(EncodedContent {
                content,
                payload,
                source_refs,
            });
            if bytes >= BATCH_BYTES || batch.len() == PG_BIND_LIMIT / 7 {
                break;
            }
        }
        let mut query = QueryBuilder::<Postgres>::new(include_str!("contents_head.sql"));
        query.push_values(&batch, |mut row, encoded| {
            row.push_bind(Uuid::now_v7())
                .push_bind(input.flow_run_id)
                .push_bind(scope_id)
                .push_bind(encoded.content.trace_node_id)
                .push_bind(&encoded.content.content_kind)
                .push_bind(&encoded.payload)
                .push_bind(&encoded.source_refs);
        });
        query.push(include_str!("contents_tail.sql"));
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}
