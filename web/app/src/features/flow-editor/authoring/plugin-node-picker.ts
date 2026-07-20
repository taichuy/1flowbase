import type { ConsoleNodeContributionEntry } from '@1flowbase/api-client';

import type { BuiltinNodePickerOption } from './node-picker';

export interface PluginContributionPickerOption {
  kind: 'plugin_contribution';
  label: string;
  contribution: ConsoleNodeContributionEntry;
  disabled: boolean;
  disabledReason: string | null;
}

export type NodePickerOption =
  | BuiltinNodePickerOption
  | PluginContributionPickerOption;

const DEPENDENCY_STATUS_LABELS: Record<string, string> = {
  missing_plugin: 'Plugin is not installed',
  version_mismatch: 'Plugin version does not match',
  disabled_plugin: 'Plugin is disabled'
};

export function toPluginContributionPickerOption(
  contribution: ConsoleNodeContributionEntry
): PluginContributionPickerOption {
  return {
    kind: 'plugin_contribution',
    label: contribution.title,
    contribution,
    disabled: contribution.dependency_status !== 'ready',
    disabledReason:
      contribution.dependency_status === 'ready'
        ? null
        : (DEPENDENCY_STATUS_LABELS[contribution.dependency_status] ??
          'Plugin node is unavailable')
  };
}

export function getNodePickerOptionKey(option: NodePickerOption) {
  return option.kind === 'builtin'
    ? option.type
    : `${option.contribution.plugin_id}:${option.contribution.contribution_code}`;
}

export function getNodePickerOptionNodeType(option: NodePickerOption) {
  return option.kind === 'builtin' ? option.type : 'plugin_node';
}

export function getNodePickerOptionDescription(option: NodePickerOption) {
  return option.kind === 'builtin'
    ? option.description
    : (option.disabledReason ?? option.contribution.description ?? null);
}
