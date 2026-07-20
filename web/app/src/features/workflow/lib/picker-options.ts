import '../register';

import type { ConsoleNodeContributionEntry } from '@1flowbase/api-client';
import {
  buildBuiltinNodePickerOptions,
  SHARED_EXECUTION_NODE_PICKER_TYPES,
  type BuiltinNodePickerOption
} from '../../flow-editor/authoring/node-picker';
import {
  toPluginContributionPickerOption,
  type NodePickerOption
} from '../../flow-editor/authoring/plugin-node-picker';

export const WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS: BuiltinNodePickerOption[] =
  buildBuiltinNodePickerOptions([
    'workflow_start',
    'workflow_end',
    ...SHARED_EXECUTION_NODE_PICKER_TYPES
  ]);

export function buildWorkflowNodePickerOptions(
  contributions: ConsoleNodeContributionEntry[]
): NodePickerOption[] {
  return [
    ...WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS,
    ...contributions.map(toPluginContributionPickerOption)
  ];
}
