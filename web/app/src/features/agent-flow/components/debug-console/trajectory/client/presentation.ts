import type { ClientTrajectoryStep } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
export function categoryLabel(category: string): string {
  switch (category) {
    case 'request':
      return i18nText('agentFlow', 'client_trajectory.request');
    case 'system':
      return i18nText('agentFlow', 'client_trajectory.system');
    case 'user':
      return i18nText('agentFlow', 'auto.user_input');
    case 'assistant':
      return i18nText('agentFlow', 'client_trajectory.assistant');
    case 'reasoning':
      return i18nText('agentFlow', 'auto.reasoning');
    case 'tool_definition':
      return i18nText('agentFlow', 'client_trajectory.tool_definition');
    case 'tool_call':
      return i18nText('agentFlow', 'auto.tool_call');
    case 'tool_result':
      return i18nText('agentFlow', 'client_trajectory.tool_result');
    case 'usage':
      return i18nText('agentFlow', 'client_trajectory.usage');
    case 'error':
      return i18nText('agentFlow', 'auto.error');
    default:
      return category;
  }
}
export function sectionLabel(section: string): string {
  switch (section) {
    case 'overview':
      return i18nText('agentFlow', 'trajectory.overview');
    case 'parameters':
      return i18nText('agentFlow', 'auto.parameters');
    case 'result':
      return i18nText('agentFlow', 'client_trajectory.result');
    case 'schema':
      return i18nText('agentFlow', 'client_trajectory.tool_definition');
    case 'timing':
      return i18nText('agentFlow', 'trajectory.time_axis');
    case 'usage':
      return i18nText('agentFlow', 'client_trajectory.usage');
    case 'raw':
      return i18nText('agentFlow', 'trajectory.protocol_integrity');
    default:
      return section;
  }
}
export function clientLane(step: ClientTrajectoryStep) {
  return step.category.startsWith('tool')
    ? 'tool'
    : ['request', 'user', 'system'].includes(step.category)
      ? 'input'
      : 'model';
}
