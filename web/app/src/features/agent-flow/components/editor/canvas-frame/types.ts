import type {
  ConsoleApplicationType,
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  ConsoleNodeContributionEntry,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';

export interface AgentFlowCanvasFrameProps {
  applicationId: string;
  applicationName: string;
  applicationType?: ConsoleApplicationType;
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeContributions: ConsoleNodeContributionEntry[];
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
}
