import { registerNodeDefinition } from '../flow-editor/authoring/node-definition-registry';
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
