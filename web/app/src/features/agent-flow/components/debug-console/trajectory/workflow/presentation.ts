import type { WorkflowTrajectoryIndexItem } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
import { invocationKey, stepLabel, stepLane } from '../trajectory-presentation';

export function workflowNodeName(
  event: {
    node_alias: string | null;
    node_run_id: string | null;
  },
  language?: string
) {
  if (event.node_run_id === null)
    return i18nText('agentFlow', 'auto.workflow', { lng: language });
  return (
    event.node_alias ||
    i18nText('agentFlow', 'trajectory.node_name_not_recorded', {
      lng: language
    })
  );
}
export function workflowGroupKey(event: WorkflowTrajectoryIndexItem) {
  return event.native_step
    ? invocationKey(event.native_step)
    : `${event.flow_run_id}:${event.task_run_id}:${event.node_run_id}:${event.category}`;
}
export function workflowEventLane(event: WorkflowTrajectoryIndexItem) {
  return event.native_step
    ? stepLane(event.native_step)
    : event.category === 'tools'
      ? 'tool'
      : event.category === 'nodes'
        ? 'input'
        : 'model';
}
export function workflowEventLabel(
  event: WorkflowTrajectoryIndexItem,
  language?: string
): string {
  if (event.native_step) return stepLabel(event.native_step, language);
  switch (event.event_type) {
    case 'flow_run_started':
      return i18nText('agentFlow', 'trajectory.event_flow_run_started', {
        lng: language
      });
    case 'flow_run_execution_started':
      return i18nText(
        'agentFlow',
        'trajectory.event_flow_run_execution_started',
        { lng: language }
      );
    case 'flow_run_resumed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_resumed', {
        lng: language
      });
    case 'flow_run_succeeded':
      return i18nText('agentFlow', 'trajectory.event_flow_run_succeeded', {
        lng: language
      });
    case 'flow_run_failed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_failed', {
        lng: language
      });
    case 'flow_run_cancelled':
      return i18nText('agentFlow', 'trajectory.event_flow_run_cancelled', {
        lng: language
      });
    case 'flow_run_completed':
      return i18nText('agentFlow', 'trajectory.event_flow_run_completed', {
        lng: language
      });
    case 'flow_run_paused':
      return i18nText('agentFlow', 'trajectory.event_flow_run_paused', {
        lng: language
      });
    case 'flow_run_waiting_callback':
      return i18nText('agentFlow', 'trajectory.event_waiting_callback', {
        lng: language
      });
    case 'flow_run_waiting_human':
      return i18nText('agentFlow', 'trajectory.event_waiting_human', {
        lng: language
      });
    case 'public_run_callback_cancelled':
      return i18nText(
        'agentFlow',
        'trajectory.event_public_run_callback_cancelled',
        { lng: language }
      );
    case 'tool_callback_completed':
      return i18nText('agentFlow', 'trajectory.event_tool_callback_completed', {
        lng: language
      });
    case 'tool_callback_failed':
      return i18nText('agentFlow', 'trajectory.event_tool_callback_failed', {
        lng: language
      });
    case 'data_model_side_effect_confirmed':
      return i18nText(
        'agentFlow',
        'trajectory.event_data_model_side_effect_confirmed',
        { lng: language }
      );
    case 'flow_finished':
      return i18nText('agentFlow', 'trajectory.event_flow_finished', {
        lng: language
      });
    case 'flow_failed':
      return i18nText('agentFlow', 'auto.execution_failed', { lng: language });
    case 'flow_cancelled':
      return i18nText('agentFlow', 'auto.execution_cancel', { lng: language });
    case 'flow_incomplete':
      return i18nText('agentFlow', 'trajectory.event_flow_incomplete', {
        lng: language
      });
    case 'waiting_callback':
      return i18nText('agentFlow', 'trajectory.event_waiting_callback', {
        lng: language
      });
    case 'waiting_human':
      return i18nText('agentFlow', 'trajectory.event_waiting_human', {
        lng: language
      });
    case 'assistant_tool_call_started':
      return i18nText(
        'agentFlow',
        'trajectory.event_assistant_tool_call_started',
        { lng: language }
      );
    case 'assistant_tool_call_finished':
      return i18nText(
        'agentFlow',
        'trajectory.event_assistant_tool_call_finished',
        { lng: language }
      );

    case 'round_started':
      return i18nText('agentFlow', 'trajectory.container_round_started', {
        lng: language
      });
    case 'round_finished':
      return i18nText('agentFlow', 'trajectory.container_round_finished', {
        lng: language
      });
    case 'node_started':
      return i18nText('agentFlow', 'trajectory.node_started', {
        lng: language
      });
    case 'node_finished':
      return i18nText('agentFlow', 'trajectory.node_finished', {
        lng: language
      });
    case 'tool_call':
      return i18nText('agentFlow', 'trajectory.tool_request', {
        lng: language
      });
    case 'tool_result':
      return i18nText('agentFlow', 'trajectory.submitted_result', {
        lng: language
      });
    default:
      switch (event.category) {
        case 'nodes':
          return i18nText('agentFlow', 'trajectory.node_events', {
            lng: language
          });
        case 'requests':
          return i18nText('agentFlow', 'client_trajectory.request', {
            lng: language
          });
        case 'tools':
          return i18nText('agentFlow', 'auto.tools', { lng: language });
        case 'rounds':
          return i18nText('agentFlow', 'trajectory.round_activities', {
            lng: language
          });
        case 'agents':
          return i18nText('agentFlow', 'trajectory.agent_activities', {
            lng: language
          });
        default:
          return event.category;
      }
  }
}

export function workflowSectionLabel(kind: string, language?: string): string {
  switch (kind) {
    case 'input':
      return i18nText('agentFlow', 'auto.input', { lng: language });
    case 'output':
      return i18nText('agentFlow', 'trajectory.section_output', {
        lng: language
      });
    case 'error':
      return i18nText('agentFlow', 'trajectory.protocol_error', {
        lng: language
      });
    case 'node':
      return i18nText('agentFlow', 'trajectory.workflow_node', {
        lng: language
      });
    default:
      return i18nText('agentFlow', 'trajectory.step_detail', { lng: language });
  }
}
