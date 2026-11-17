import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationNodeCatalog,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';
import type { ReactNode } from 'react';

import { FlowEditorKernel } from '../../../flow-editor';
import { AgentFlowCanvasFrame } from './AgentFlowCanvasFrame';
import './styles/index.css';

const EMPTY_ENVIRONMENT_VARIABLES: ConsoleApplicationEnvironmentVariable[] = [];
const EMPTY_NODE_CATALOG: ConsoleApplicationNodeCatalog = { nodes: [] };

export interface AgentFlowEditorAssemblyProps {
  applicationId: string;
  applicationName: string;
  initialState: ConsoleApplicationOrchestrationState;
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeCatalog?: ConsoleApplicationNodeCatalog;
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
  topSlot?: ReactNode;
}

export function AgentFlowEditorAssembly({
  applicationId,
  applicationName,
  initialState,
  initialEnvironmentVariables = EMPTY_ENVIRONMENT_VARIABLES,
  nodeCatalog = EMPTY_NODE_CATALOG,
  saveDraftOverride,
  restoreVersionOverride,
  topSlot
}: AgentFlowEditorAssemblyProps) {
  return (
    <div data-testid="agent-flow-editor-assembly">
      <FlowEditorKernel
        key={applicationId}
        initialState={initialState}
        slots={{
          toolbar: topSlot,
          canvas: (
            <AgentFlowCanvasFrame
              applicationId={applicationId}
              applicationName={applicationName}
              initialEnvironmentVariables={initialEnvironmentVariables}
              nodeCatalog={nodeCatalog}
              saveDraftOverride={saveDraftOverride}
              restoreVersionOverride={restoreVersionOverride}
            />
          )
        }}
      />
    </div>
  );
}
