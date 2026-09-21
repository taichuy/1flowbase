import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import { i18nText } from '../../../../../shared/i18n/text';

export function stepLabel(step: ProviderTrajectoryStep): string {
  switch (step.metadata.kind) {
    case 'model_call':
      return i18nText('agentFlow', 'trajectory.model_call');
    case 'model_reply':
      return i18nText('agentFlow', 'trajectory.model_reply');
    case 'tool_call':
      return i18nText('agentFlow', 'trajectory.tool_request');
    case 'tool_result':
      return i18nText('agentFlow', 'trajectory.submitted_result');
    case 'error':
      return i18nText('agentFlow', 'trajectory.protocol_error');
    case 'observation_gap':
      return i18nText('agentFlow', 'trajectory.semantic_gap');
  }
}
export function stepLane(
  step: ProviderTrajectoryStep
): 'input' | 'model' | 'tool' {
  if (step.metadata.kind === 'model_call') return 'input';
  if (
    step.metadata.kind === 'tool_call' ||
    step.metadata.kind === 'tool_result'
  )
    return 'tool';
  return 'model';
}
export function invocationKey(step: ProviderTrajectoryStep) {
  return `${step.metadata.flow_run_id}:${step.metadata.node_run_id}:${step.metadata.invocation_id}:${step.metadata.provider_attempt_index}`;
}
export function integrityLabel(value?: string) {
  switch (value) {
    case 'complete':
      return i18nText('agentFlow', 'trajectory.complete');
    case 'incomplete':
      return i18nText('agentFlow', 'trajectory.incomplete');
    case 'pending':
      return i18nText('agentFlow', 'trajectory.pending');
    default:
      return i18nText('agentFlow', 'trajectory.unavailable');
  }
}
