import type {
  ConsoleApplicationEnvironmentVariable,
  ConsoleApplicationOrchestrationState,
  ConsoleNodeContributionEntry,
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
  nodeContributions?: ConsoleNodeContributionEntry[];
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
  nodeContributions = [],
  saveDraftOverride,
  restoreVersionOverride
}: WorkflowEditorAssemblyProps) {
  return (
    <div data-testid="workflow-editor-assembly">
      <FlowEditorKernel
        initialState={initialState}
        slots={{
          canvas: (
            <WorkflowCanvasFrame
              applicationId={applicationId}
              applicationName={applicationName}
              initialEnvironmentVariables={initialEnvironmentVariables}
              nodeContributions={nodeContributions}
              triggerContext={workflowTriggerContext}
              saveDraftOverride={saveDraftOverride}
              restoreVersionOverride={restoreVersionOverride}
            />
          )
        }}
      />
    </div>
  );
}
