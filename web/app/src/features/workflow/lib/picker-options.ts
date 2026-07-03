import '../register';

import type { AgentFlowNodeContributionEntry } from '../../agent-flow/api/node-contributions';
import {
  GENERAL_EXECUTION_NODE_PICKER_TYPES,
  buildBuiltinNodePickerOptions,
  toPluginContributionPickerOption,
  type BuiltinNodePickerOption,
  type NodePickerOption
} from '../../agent-flow/lib/plugin-node-definitions';

export const WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS: BuiltinNodePickerOption[] =
  buildBuiltinNodePickerOptions([
    'workflow_start',
    'workflow_end',
    ...GENERAL_EXECUTION_NODE_PICKER_TYPES
  ]);

export function buildWorkflowNodePickerOptions(
  contributions: AgentFlowNodeContributionEntry[]
): NodePickerOption[] {
  return [
    ...WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS,
    ...contributions.map(toPluginContributionPickerOption)
  ];
}
