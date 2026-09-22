import type {
  ProviderTrajectoryStep,
  WorkflowTrajectoryEvent,
  WorkflowTrajectoryPage
} from '@1flowbase/api-client';
export function workflowNative(
  step: ProviderTrajectoryStep,
  node_alias = '执行时节点名称'
): WorkflowTrajectoryEvent {
  return {
    event_id: `native:${step.event_id}`,
    event_sequence: step.event_sequence,
    event_type: step.event_type,
    created_at: step.created_at,
    category: step.metadata.kind.startsWith('tool') ? 'tools' : 'requests',
    flow_run_id: step.metadata.flow_run_id,
    task_run_id: 'task-root',
    parent_task_run_id: null,
    node_run_id: step.metadata.node_run_id,
    node_id: step.metadata.node_id,
    node_alias,
    node_type: 'llm',
    status: step.metadata.status,
    preview: step.metadata.preview ?? '',
    native_step: step
  };
}
export function workflowPage(
  items: WorkflowTrajectoryEvent[],
  next_cursor: string | null = null
): WorkflowTrajectoryPage {
  return {
    items,
    next_cursor,
    nodes: [
      {
        flow_run_id: 'run-current',
        node_run_id: 'node-current',
        node_id: 'llm',
        node_alias: '执行时节点名称',
        node_type: 'llm'
      }
    ],
    time_start: '2026-09-22T00:00:00Z',
    time_end: '2026-09-22T01:00:00Z'
  };
}
