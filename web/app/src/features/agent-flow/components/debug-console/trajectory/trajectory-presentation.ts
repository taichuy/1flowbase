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

export function purposeLabel(purpose?: string) {
  switch (purpose) {
    case 'prewarm':
      return i18nText('agentFlow', 'trajectory.purpose_prewarm');
    case 'generate':
      return i18nText('agentFlow', 'trajectory.purpose_generate');
    case 'tool_resume':
      return i18nText('agentFlow', 'trajectory.purpose_tool_resume');
    case 'compact':
      return i18nText('agentFlow', 'trajectory.purpose_compact');
    default:
      return i18nText('agentFlow', 'trajectory.purpose_unknown');
  }
}
export function nativeSectionLabel(kind: string) {
  switch (kind) {
    case 'system':
      return i18nText('agentFlow', 'trajectory.section_system');
    case 'context':
      return i18nText('agentFlow', 'trajectory.section_context');
    case 'tools':
      return i18nText('agentFlow', 'trajectory.section_tools');
    case 'configuration':
      return i18nText('agentFlow', 'trajectory.section_configuration');
    case 'output':
      return i18nText('agentFlow', 'trajectory.section_output');
    case 'error':
      return i18nText('agentFlow', 'trajectory.protocol_error');
    default:
      return kind;
  }
}

// Presentation only: retain the backend section kind and value unchanged.
export function eventSectionTab(
  kind: string
): 'input' | 'process' | 'output' | 'metadata' {
  switch (kind) {
    case 'input':
    case 'system':
    case 'context':
      return 'input';
    case 'output':
      return 'output';
    case 'node':
      return 'metadata';
    default:
      return 'process';
  }
}
