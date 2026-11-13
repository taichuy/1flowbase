import type {
  ConsoleApplicationRunDetail,
  ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';

export interface WorkflowRunState {
  runId: string | null;
  status: string;
  executions: Record<string, { node_id: string; status: string }>;
  output: Record<string, unknown> | null;
  failure: string | null;
  connectionLost: boolean;
}

export function emptyWorkflowRunState(): WorkflowRunState {
  return {
    runId: null,
    status: 'idle',
    executions: {},
    output: null,
    failure: null,
    connectionLost: false
  };
}

export function isWorkflowRunActive(state: WorkflowRunState) {
  return !['idle', 'succeeded', 'failed', 'cancelled', 'incomplete'].includes(
    state.status
  );
}

export function workflowNodeRunStatuses(state: WorkflowRunState) {
  const statuses: Record<string, 'running' | 'succeeded' | 'failed'> = {};
  for (const execution of Object.values(state.executions)) {
    const status = execution.status;
    const next = ['running', 'waiting_human', 'waiting_callback'].includes(
      status
    )
      ? 'running'
      : status === 'succeeded' || status === 'failed'
        ? status
        : undefined;
    if (!next) continue;
    const current = statuses[execution.node_id];
    if (current === 'running' || (current === 'failed' && next !== 'running'))
      continue;
    statuses[execution.node_id] = next;
  }
  return statuses;
}

export function applyWorkflowRunEvent(
  state: WorkflowRunState,
  event: ConsoleFlowDebugStreamEvent
): WorkflowRunState {
  const runId = 'run_id' in event ? (event.run_id ?? state.runId) : state.runId;
  if (event.type === 'flow_accepted' || event.type === 'flow_started') {
    return { ...state, runId, status: event.status, connectionLost: false };
  }
  if (event.type === 'node_started' || event.type === 'node_finished') {
    return {
      ...state,
      runId,
      connectionLost: false,
      executions: {
        ...state.executions,
        [event.node_run_id]: {
          node_id: event.node_id,
          status: event.type === 'node_started' ? 'running' : event.status
        }
      }
    };
  }
  if (event.type === 'waiting_human' || event.type === 'waiting_callback') {
    return { ...state, runId, status: event.status };
  }
  if (
    event.type === 'flow_finished' ||
    event.type === 'flow_incomplete' ||
    event.type === 'flow_failed' ||
    event.type === 'flow_cancelled'
  ) {
    return {
      ...state,
      runId,
      connectionLost: false,
      status: event.type === 'flow_failed' ? 'failed' : event.status,
      output:
        event.type === 'flow_finished' || event.type === 'flow_incomplete'
          ? event.output
          : null,
      failure: event.type === 'flow_failed' ? event.error : null,
      // A terminal flow event stops outstanding spinners without inventing success.
      executions: Object.fromEntries(
        Object.entries(state.executions).map(([id, execution]) => [
          id,
          ['running', 'waiting_human', 'waiting_callback'].includes(
            execution.status
          )
            ? { ...execution, status: 'cancelled' }
            : execution
        ])
      )
    };
  }
  return state;
}

export function workflowRunSnapshot(
  detail: ConsoleApplicationRunDetail
): WorkflowRunState {
  return {
    runId: detail.flow_run.id,
    status: detail.flow_run.status,
    executions: Object.fromEntries(
      detail.node_runs.map((node) => [
        node.id,
        { node_id: node.node_id, status: node.status }
      ])
    ),
    output: detail.flow_run.output_payload,
    failure: null,
    connectionLost: false
  };
}
