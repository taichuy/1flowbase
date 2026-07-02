import type {
  ConsoleApplicationType,
  ConsoleWorkflowTriggerType,
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
  workflowTriggerType?: ConsoleWorkflowTriggerType | null;
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
  workflowTriggerType = null,
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
        workflowTriggerType={workflowTriggerType}
        initialEnvironmentVariables={initialEnvironmentVariables}
        nodeContributions={nodeContributions}
        saveDraftOverride={saveDraftOverride}
        restoreVersionOverride={restoreVersionOverride}
      />
    </AgentFlowEditorStoreProvider>
  );
}
