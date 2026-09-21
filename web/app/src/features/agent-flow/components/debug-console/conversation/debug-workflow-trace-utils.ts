import type { AgentFlowTraceItem } from '../../../api/runtime';
import { i18nText } from '../../../../../shared/i18n/text';

export interface AgentFlowTraceDisplayGroup {
  key: string;
  item: AgentFlowTraceItem;
  items: AgentFlowTraceItem[];
}

export function getTraceItemKey(item: AgentFlowTraceItem) {
  return item.nodeRunId ?? item.nodeId;
}

export function nodeDisplayName(item: AgentFlowTraceItem) {
  if (item.nodeType === 'start') {
    return i18nText('agentFlow', 'auto.user_input');
  }

  if (item.nodeType === 'answer') {
    return i18nText('agentFlow', 'auto.reply_directly');
  }

  if (item.nodeType === 'fusion') {
    return i18nText('agentFlow', 'auto.tool_mode_fusion');
  }

  if (item.nodeType === 'route') {
    return i18nText('agentFlow', 'auto.tool_mode_agent');
  }

  return item.nodeAlias;
}

export function groupTraceItemsForDisplay(items: AgentFlowTraceItem[]): AgentFlowTraceDisplayGroup[] {
  return items.map((item) => ({ key: getTraceItemKey(item), item, items: [item] }));
}
