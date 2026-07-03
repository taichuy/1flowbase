import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  ConsoleNodeContributionEntry,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';

import type { AgentFlowNodeContributionEntry } from '../../../api/node-contributions';
import type { NodePickerOption } from '../../../lib/plugin-node-definitions';

export interface AgentFlowEditorCapabilities {
  conversationDebug: boolean;
  conversationVariables: boolean;
  systemVariables: boolean;
}

export const DEFAULT_EDITOR_CAPABILITIES: AgentFlowEditorCapabilities = {
  conversationDebug: true,
  conversationVariables: true,
  systemVariables: true
};

export interface AgentFlowCanvasFrameProps {
  applicationId: string;
  applicationName: string;
  workflowTriggerContext?: unknown;
  capabilities?: AgentFlowEditorCapabilities;
  nodePickerOptionsBuilder?: (
    contributions: AgentFlowNodeContributionEntry[]
  ) => NodePickerOption[];
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeContributions: ConsoleNodeContributionEntry[];
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
}
