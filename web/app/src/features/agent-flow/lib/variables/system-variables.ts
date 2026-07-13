import type { FlowNodeOutputDocument } from '@1flowbase/flow-schema';
import { i18nText } from '../../../../shared/i18n/text';

export interface AgentFlowSystemVariable extends FlowNodeOutputDocument {
  description: string;
}

export const systemVariableNodeId = 'sys';

const workflowSystemVariableKeys = new Set([
  'application_id',
  'workflow_id',
  'workflow_run_id'
]);

export const agentFlowSystemVariables: AgentFlowSystemVariable[] = [
  {
    key: 'conversation_id',
    title: 'sys.conversation_id',
    valueType: 'string',
    description: i18nText('agentFlow', 'auto.system_variable_conversation_id')
  },
  {
    key: 'dialog_count',
    title: 'sys.dialog_count',
    valueType: 'number',
    description: i18nText('agentFlow', 'auto.system_variable_dialog_count')
  },
  {
    key: 'user_id',
    title: 'sys.user_id',
    valueType: 'string',
    description: i18nText('agentFlow', 'auto.system_variable_user_id')
  },
  {
    key: 'application_id',
    title: 'sys.application_id',
    valueType: 'string',
    description: i18nText('agentFlow', 'auto.system_variable_application_id')
  },
  {
    key: 'workflow_id',
    title: 'sys.workflow_id',
    valueType: 'string',
    description: i18nText('agentFlow', 'auto.system_variable_workflow_id')
  },
  {
    key: 'workflow_run_id',
    title: 'sys.workflow_run_id',
    valueType: 'string',
    description: i18nText('agentFlow', 'auto.system_variable_workflow_run_id')
  },
  {
    key: 'model_parameters',
    title: 'sys.model_parameters',
    valueType: 'json',
    description: i18nText('agentFlow', 'auto.system_variable_model_parameters')
  }
];

export const workflowSystemVariables = agentFlowSystemVariables.filter(
  (variable) => workflowSystemVariableKeys.has(variable.key)
);
