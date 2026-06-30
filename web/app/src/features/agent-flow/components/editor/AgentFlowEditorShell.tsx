import type {
  ConsoleApplicationType,
  ConsoleNodeContributionEntry,
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';

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
}

export function AgentFlowEditorShell({
  applicationId,
  applicationName,
  applicationType = 'agent_flow',
  initialState,
  initialEnvironmentVariables = [],
  nodeContributions = [],
  saveDraftOverride,
  restoreVersionOverride
}: AgentFlowEditorShellProps) {
  return (
    <AgentFlowEditorStoreProvider initialState={initialState}>
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
