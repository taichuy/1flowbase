use super::*;

pub(in crate::orchestration_runtime) async fn freeze_failover_queue_routes<R>(
    repository: &R,
    _workspace_id: Uuid,
    compiled_plan: &mut orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()>
where
    R: ModelProviderRepository + PluginRepository,
{
    for node in compiled_plan.nodes.values_mut() {
        let Some(runtime) = node.llm_runtime.as_mut() else {
            continue;
        };
        let Some(routing) = runtime.routing.as_mut() else {
            continue;
        };
        if routing.routing_mode
            != orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue
            || !routing.queue_targets.is_empty()
        {
            continue;
        }

        let queue_template_id = routing
            .queue_template_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        let queue = repository
            .get_failover_queue_template(queue_template_id)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        if queue.status != "active" {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        }
        let items = repository
            .list_failover_queue_items(queue_template_id)
            .await?;
        let snapshot_items = items
            .iter()
            .cloned()
            .map(FailoverQueueSnapshotItem::from)
            .collect::<Vec<_>>();
        let snapshot = repository
            .create_failover_queue_snapshot(&crate::ports::CreateModelFailoverQueueSnapshotInput {
                snapshot_id: Uuid::now_v7(),
                queue_template_id,
                version: queue.version,
                items: freeze_queue_items(&snapshot_items),
            })
            .await?;
        routing.queue_snapshot_id = Some(snapshot.id.to_string());
        let provider_display_names = routing
            .queue_targets
            .iter()
            .map(|target| {
                (
                    target.provider_instance_id.clone(),
                    target.provider_instance_display_name.clone(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        routing.queue_targets = snapshot_items
            .into_iter()
            .filter(|item| item.enabled)
            .map(
                |item| orchestration_runtime::compiled_plan::CompiledLlmRouteTarget {
                    provider_instance_id: item.provider_instance_id.to_string(),
                    provider_instance_display_name: provider_display_names
                        .get(&item.provider_instance_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                    provider_code: item.provider_code,
                    protocol: item.protocol,
                    upstream_model_id: item.upstream_model_id,
                },
            )
            .collect();
        let Some(first_target) = routing.queue_targets.first() else {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        };
        runtime.provider_instance_id = first_target.provider_instance_id.clone();
        runtime.provider_code = first_target.provider_code.clone();
        runtime.protocol = first_target.protocol.clone();
        runtime.model = first_target.upstream_model_id.clone();
    }

    Ok(())
}
