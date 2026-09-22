import { createContext } from 'react';
import type { ConversationLogTraceNodeSummary } from '../../conversation-log-trace-model';

export type ActivityCategory = 'tools' | 'rounds' | 'agents';
// These are existing backend projection kinds, not inferred protocol events.
export function activityCategory(
  node: ConversationLogTraceNodeSummary
): ActivityCategory | null {
  switch (node.node_kind) {
    case 'tool_group':
    case 'tool_callback':
      return 'tools';
    case 'agent_group':
    case 'child_task':
      return 'agents';
    case 'round_group':
    case 'task_round':
    case 'stitched_context':
    case 'stitched_run':
      return 'rounds';
    default:
      return null;
  }
}

// The original detail renderer remains the owner of node I/O and source relations.
export const TraceActivityDetailContext = createContext(false);
