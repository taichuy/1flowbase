import type {
  FlowAuthoringDocument,
  FlowNodeOutputDocument
} from '@1flowbase/flow-schema';

import {
  listAuthoringVariableOptions,
  type AuthoringVariableSource
} from '../../flow-editor/authoring/variables';

const workflowSystemOutputs = [
  { key: 'application_id', title: 'sys.application_id', valueType: 'string' },
  { key: 'workflow_id', title: 'sys.workflow_id', valueType: 'string' },
  { key: 'workflow_run_id', title: 'sys.workflow_run_id', valueType: 'string' }
] satisfies FlowNodeOutputDocument[];

function workflowTriggerOutputs(triggerType: unknown) {
  const outputs: FlowNodeOutputDocument[] = [
    { key: 'type', title: 'trigger.type', valueType: 'string' }
  ];
  if (triggerType === 'schedule') {
    outputs.push(
      {
        key: 'scheduled_at',
        title: 'trigger.scheduled_at',
        valueType: 'string'
      },
      { key: 'timezone', title: 'trigger.timezone', valueType: 'string' }
    );
  }
  return outputs;
}

export function getWorkflowVariableSources(
  workflowTriggerContext: unknown
): AuthoringVariableSource[] {
  const triggerType =
    typeof workflowTriggerContext === 'object' &&
    workflowTriggerContext !== null &&
    'triggerType' in workflowTriggerContext
      ? workflowTriggerContext.triggerType
      : null;

  return [
    { id: 'sys', label: 'System variables', outputs: workflowSystemOutputs },
    {
      id: 'trigger',
      label: 'Trigger variables',
      outputs: workflowTriggerOutputs(triggerType)
    }
  ];
}

export function listWorkflowVariableOptions(
  document: FlowAuthoringDocument,
  nodeId: string,
  workflowTriggerContext: unknown
) {
  return listAuthoringVariableOptions(
    document,
    nodeId,
    getWorkflowVariableSources(workflowTriggerContext)
  );
}
