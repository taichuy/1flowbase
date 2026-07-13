import type { FlowNodeOutputDocument } from '@1flowbase/flow-schema';

export const workflowTriggerVariableNodeId = 'trigger';

const workflowTriggerTypeVariable = {
  key: 'type',
  title: 'trigger.type',
  valueType: 'string'
} satisfies FlowNodeOutputDocument;

const workflowScheduleTriggerVariables = [
  {
    key: 'scheduled_at',
    title: 'trigger.scheduled_at',
    valueType: 'string'
  },
  {
    key: 'timezone',
    title: 'trigger.timezone',
    valueType: 'string'
  }
] satisfies FlowNodeOutputDocument[];

export function getWorkflowTriggerVariables(triggerType: unknown) {
  return triggerType === 'schedule'
    ? [workflowTriggerTypeVariable, ...workflowScheduleTriggerVariables]
    : [workflowTriggerTypeVariable];
}
