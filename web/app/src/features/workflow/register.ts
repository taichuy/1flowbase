import { registerNodeDefinition } from '../agent-flow/lib/node-definitions/registry';
import { registerAgentFlowRenderers } from '../agent-flow/schema/agent-flow-renderer-registry';
import { registerContractFieldRenderers } from '../agent-flow/schema/node-schema-fragments';
import { WorkflowTriggerConfigField } from './components/WorkflowTriggerConfigField';
import { WorkflowStartCardDescription } from './components/WorkflowStartCardDescription';
import {
  createWorkflowEndContract,
  createWorkflowStartContract,
  getWorkflowStartNodeVariableOutputs,
  workflowEndNodeMeta,
  workflowStartNodeMeta
} from './lib/node-definitions';

registerNodeDefinition('workflow_start', {
  contract: createWorkflowStartContract(),
  meta: workflowStartNodeMeta,
  variableOutputs: getWorkflowStartNodeVariableOutputs,
  cardDescription: WorkflowStartCardDescription,
  suppressGeneratedOutputVariables: true
});

registerNodeDefinition('workflow_end', {
  contract: createWorkflowEndContract(),
  meta: workflowEndNodeMeta,
  editableOutputContract: true
});

registerAgentFlowRenderers({
  fields: {
    workflow_trigger_config: WorkflowTriggerConfigField
  }
});

registerContractFieldRenderers(['workflow_trigger_config']);
