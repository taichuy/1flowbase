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
  initialEnvironmentVariables = [],
  nodeCatalog = { nodes: [] },
  saveDraftOverride,
  restoreVersionOverride,
  topSlot
}: AgentFlowEditorAssemblyProps) {
  return (
    <div data-testid="agent-flow-editor-assembly">
      <FlowEditorKernel
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
