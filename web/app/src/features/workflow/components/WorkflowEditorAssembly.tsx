import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationNodeCatalog,
  ConsoleApplicationOrchestrationState,
  SaveConsoleApplicationDraftInput
} from '@1flowbase/api-client';

import { FlowEditorKernel } from '../../flow-editor';
import type { WorkflowTriggerContext } from '../lib/trigger-context';
import { WorkflowCanvasFrame } from './WorkflowCanvasFrame';
import '../../agent-flow/components/editor/styles/index.css';

interface WorkflowEditorAssemblyProps {
  applicationId: string;
  applicationName: string;
  initialState: ConsoleApplicationOrchestrationState;
  workflowTriggerContext: WorkflowTriggerContext;
  initialEnvironmentVariables?: ConsoleApplicationEnvironmentVariable[];
  nodeCatalog?: ConsoleApplicationNodeCatalog;
  saveDraftOverride?: (
    input: SaveConsoleApplicationDraftInput
  ) => Promise<ConsoleApplicationOrchestrationState>;
  restoreVersionOverride?: (
    versionId: string
  ) => Promise<ConsoleApplicationOrchestrationState>;
}

export function WorkflowEditorAssembly({
  applicationId,
  applicationName,
  initialState,
  workflowTriggerContext,
  initialEnvironmentVariables = [],
  nodeCatalog = { nodes: [] },
  saveDraftOverride,
  restoreVersionOverride
}: WorkflowEditorAssemblyProps) {
  return (
    <FlowEditorKernel
      initialState={initialState}
      slots={{
        canvas: (
          <WorkflowCanvasFrame
            applicationId={applicationId}
            applicationName={applicationName}
            initialEnvironmentVariables={initialEnvironmentVariables}
            nodeCatalog={nodeCatalog}
            triggerContext={workflowTriggerContext}
            saveDraftOverride={saveDraftOverride}
            restoreVersionOverride={restoreVersionOverride}
          />
        )
      }}
    />
  );
}
