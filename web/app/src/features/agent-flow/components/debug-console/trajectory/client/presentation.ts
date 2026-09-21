import type { ClientTrajectoryStep } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
export function categoryLabel(category: string): string {
  switch (category) {
    case 'request':
      return i18nText('agentFlow', 'clientTrajectory.request');
    case 'system':
      return i18nText('agentFlow', 'clientTrajectory.system');
    case 'user':
      return i18nText('agentFlow', 'clientTrajectory.user');
    case 'assistant':
      return i18nText('agentFlow', 'clientTrajectory.assistant');
    case 'reasoning':
      return i18nText('agentFlow', 'clientTrajectory.reasoning');
    case 'tool_definition':
      return i18nText('agentFlow', 'clientTrajectory.tool_definition');
    case 'tool_call':
      return i18nText('agentFlow', 'clientTrajectory.tool_call');
    case 'tool_result':
      return i18nText('agentFlow', 'clientTrajectory.tool_result');
    case 'usage':
      return i18nText('agentFlow', 'clientTrajectory.usage');
    case 'error':
      return i18nText('agentFlow', 'clientTrajectory.error');
    default:
      return category;
  }
}
export function sectionLabel(section: string): string {
  switch (section) {
    case 'overview':
      return i18nText('agentFlow', 'clientTrajectory.overview');
    case 'parameters':
      return i18nText('agentFlow', 'clientTrajectory.parameters');
    case 'result':
      return i18nText('agentFlow', 'clientTrajectory.result');
    case 'schema':
      return i18nText('agentFlow', 'clientTrajectory.schema');
    case 'timing':
      return i18nText('agentFlow', 'clientTrajectory.timing');
    case 'usage':
      return i18nText('agentFlow', 'clientTrajectory.usage');
    case 'raw':
      return i18nText('agentFlow', 'clientTrajectory.raw');
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
