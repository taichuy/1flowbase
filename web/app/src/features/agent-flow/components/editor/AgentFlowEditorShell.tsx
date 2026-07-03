import type {
  ConsoleNodeContributionEntry,
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';
import type { ReactNode } from 'react';

import './styles/index.css';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { AgentFlowCanvasFrame } from './AgentFlowCanvasFrame';
import type {
  AgentFlowCanvasFrameProps,
  AgentFlowEditorCapabilities
} from './canvas-frame/types';

interface AgentFlowEditorShellProps {
  applicationId: string;
  applicationName: string;
  workflowTriggerContext?: unknown;
  capabilities?: AgentFlowEditorCapabilities;
  nodePickerOptionsBuilder?: AgentFlowCanvasFrameProps['nodePickerOptionsBuilder'];
  initialState: ConsoleApplicationOrchestrationState;
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeContributions?: ConsoleNodeContributionEntry[];
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
  topSlot?: ReactNode;
}

export function AgentFlowEditorShell({
  applicationId,
  applicationName,
  workflowTriggerContext = null,
  capabilities,
  nodePickerOptionsBuilder,
  initialState,
  initialEnvironmentVariables = [],
  nodeContributions = [],
  saveDraftOverride,
  restoreVersionOverride,
  topSlot
}: AgentFlowEditorShellProps) {
  return (
    <AgentFlowEditorStoreProvider initialState={initialState}>
      {topSlot}
      <AgentFlowCanvasFrame
        applicationId={applicationId}
        applicationName={applicationName}
        workflowTriggerContext={workflowTriggerContext}
        capabilities={capabilities}
        nodePickerOptionsBuilder={nodePickerOptionsBuilder}
        initialEnvironmentVariables={initialEnvironmentVariables}
        nodeContributions={nodeContributions}
        saveDraftOverride={saveDraftOverride}
        restoreVersionOverride={restoreVersionOverride}
      />
    </AgentFlowEditorStoreProvider>
  );
}
