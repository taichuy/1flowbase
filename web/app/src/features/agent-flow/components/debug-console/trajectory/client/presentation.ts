import type { ClientTrajectoryStep } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
export function categoryLabel(category: string, language?: string): string {
  switch (category) {
    case 'request':
      return i18nText('agentFlow', 'client_trajectory.request', { lng: language });
    case 'system':
      return i18nText('agentFlow', 'client_trajectory.system', { lng: language });
    case 'user':
      return i18nText('agentFlow', 'auto.user_input', { lng: language });
    case 'assistant':
      return i18nText('agentFlow', 'client_trajectory.assistant', { lng: language });
    case 'reasoning':
      return i18nText('agentFlow', 'auto.reasoning', { lng: language });
    case 'tool_definition':
      return i18nText('agentFlow', 'client_trajectory.tool_definition', { lng: language });
    case 'tool_call':
      return i18nText('agentFlow', 'auto.tool_call', { lng: language });
    case 'tool_result':
      return i18nText('agentFlow', 'client_trajectory.tool_result', { lng: language });
    case 'usage':
      return i18nText('agentFlow', 'client_trajectory.usage', { lng: language });
    case 'error':
      return i18nText('agentFlow', 'auto.error', { lng: language });
    default:
      return category;
  }
}
export function sectionLabel(section: string, language?: string): string {
  switch (section) {
    case 'overview':
      return i18nText('agentFlow', 'trajectory.overview', { lng: language });
    case 'parameters':
      return i18nText('agentFlow', 'auto.parameters', { lng: language });
    case 'result':
      return i18nText('agentFlow', 'client_trajectory.result', { lng: language });
    case 'schema':
      return i18nText('agentFlow', 'client_trajectory.tool_definition', { lng: language });
    case 'timing':
      return i18nText('agentFlow', 'trajectory.time_axis', { lng: language });
    case 'usage':
      return i18nText('agentFlow', 'client_trajectory.usage', { lng: language });
    case 'raw':
      return i18nText('agentFlow', 'trajectory.protocol_integrity', { lng: language });
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
