import type { WorkflowTrajectoryEvent } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
import { invocationKey, stepLabel, stepLane } from '../trajectory-presentation';

export function workflowNodeName(event: {
  node_alias: string | null;
  node_run_id: string | null;
}) {
  if (event.node_run_id === null)
    return i18nText('agentFlow', 'auto.workflow');
  return (
    event.node_alias ||
    i18nText('agentFlow', 'trajectory.node_name_not_recorded')
  );
}
export function workflowGroupKey(event: WorkflowTrajectoryEvent) {
  return event.native_step
    ? invocationKey(event.native_step)
    : `${event.flow_run_id}:${event.task_run_id}:${event.node_run_id}:${event.category}`;
}
export function workflowEventLane(event: WorkflowTrajectoryEvent) {
  return event.native_step
    ? stepLane(event.native_step)
    : event.category === 'tools'
      ? 'tool'
      : event.category === 'nodes'
        ? 'input'
        : 'model';
}
export function workflowEventLabel(event: WorkflowTrajectoryEvent): string {
  if (event.native_step) return stepLabel(event.native_step);
  switch (event.event_type) {
    case 'flow_run_started':
      return i18nText('agentFlow', 'trajectory.event_flow_run_started');
    case 'flow_run_execution_started':
      return i18nText(
        'agentFlow',
        'trajectory.event_flow_run_execution_started'
      );
    case 'flow_run_resumed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_resumed');
    case 'flow_run_succeeded':
      return i18nText('agentFlow', 'trajectory.event_flow_run_succeeded');
    case 'flow_run_failed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_failed');
    case 'flow_run_cancelled':
      return i18nText('agentFlow', 'trajectory.event_flow_run_cancelled');
    case 'flow_run_completed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_completed');
    case 'flow_run_paused':
      return i18nText('agentFlow', 'trajectory.event_flow_run_paused');
    case 'flow_run_waiting_callback':
      return i18nText(
        'agentFlow',
        'trajectory.event_waiting_callback'
      );
    case 'flow_run_waiting_human':
      return i18nText('agentFlow', 'trajectory.event_waiting_human');
    case 'public_run_callback_cancelled':
      return i18nText(
        'agentFlow',
        'trajectory.event_public_run_callback_cancelled'
      );
    case 'tool_callback_completed':
      return i18nText('agentFlow', 'trajectory.event_tool_callback_completed');
    case 'tool_callback_failed':
      return i18nText('agentFlow', 'trajectory.event_tool_callback_failed');
    case 'data_model_side_effect_confirmed':
      return i18nText(
        'agentFlow',
        'trajectory.event_data_model_side_effect_confirmed'
      );
    case 'flow_finished':
      return i18nText('agentFlow', 'trajectory.event_flow_finished');
    case 'flow_failed':
      return i18nText('agentFlow', 'auto.execution_failed');
    case 'flow_cancelled':
      return i18nText('agentFlow', 'auto.execution_cancel');
    case 'flow_incomplete':
      return i18nText('agentFlow', 'trajectory.event_flow_incomplete');
    case 'waiting_callback':
      return i18nText('agentFlow', 'trajectory.event_waiting_callback');
    case 'waiting_human':
      return i18nText('agentFlow', 'trajectory.event_waiting_human');
    case 'assistant_tool_call_started':
      return i18nText(
        'agentFlow',
        'trajectory.event_assistant_tool_call_started'
      );
    case 'assistant_tool_call_finished':
      return i18nText(
        'agentFlow',
        'trajectory.event_assistant_tool_call_finished'
      );

    case 'node_started':
      return i18nText('agentFlow', 'trajectory.node_started');
    case 'node_finished':
      return i18nText('agentFlow', 'trajectory.node_finished');
    case 'tool_call':
      return i18nText('agentFlow', 'trajectory.tool_request');
    case 'tool_result':
      return i18nText('agentFlow', 'trajectory.submitted_result');
    default:
      switch (event.category) {
        case 'nodes':
          return i18nText('agentFlow', 'trajectory.node_events');
        case 'requests':
          return i18nText('agentFlow', 'client_trajectory.request');
        case 'tools':
          return i18nText('agentFlow', 'auto.tools');
        case 'rounds':
          return i18nText('agentFlow', 'trajectory.round_activities');
        case 'agents':
          return i18nText('agentFlow', 'trajectory.agent_activities');
      }
  }
}

export function workflowSectionLabel(kind: string): string {
  switch (kind) {
    case 'input':
      return i18nText('agentFlow', 'auto.input');
    case 'output':
      return i18nText('agentFlow', 'trajectory.section_output');
    case 'error':
      return i18nText('agentFlow', 'trajectory.protocol_error');
    case 'node':
      return i18nText('agentFlow', 'trajectory.workflow_node');
    default:
      return i18nText('agentFlow', 'trajectory.step_detail');
  }
}
