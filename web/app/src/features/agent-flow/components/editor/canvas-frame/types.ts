import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationNodeCatalog,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';

export interface AgentFlowCanvasFrameProps {
  applicationId: string;
  applicationName: string;
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeCatalog: ConsoleApplicationNodeCatalog;
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
}
