import type {
  ConsoleApplicationType,
  ConsoleNodeContributionEntry,
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';
import type { ReactNode } from 'react';

import './styles/index.css';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { AgentFlowCanvasFrame } from './AgentFlowCanvasFrame';

interface AgentFlowEditorShellProps {
  applicationId: string;
  applicationName: string;
  applicationType?: ConsoleApplicationType;
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
  applicationType = 'agent_flow',
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
        applicationType={applicationType}
        initialEnvironmentVariables={initialEnvironmentVariables}
        nodeContributions={nodeContributions}
        saveDraftOverride={saveDraftOverride}
        restoreVersionOverride={restoreVersionOverride}
      />
    </AgentFlowEditorStoreProvider>
  );
}
