import type {
  ConsoleApplicationNodeCatalogEntry,
  ConsoleApplicationNodeFieldContract,
  ConsolePluginNodeIdentity
} from '@1flowbase/api-client';

import {
  toBuiltinNodePickerOption,
  type BuiltinNodePickerOption
} from './node-picker';

export interface PluginContributionPickerOption {
  kind: 'plugin_contribution';
  label: string;
  description: string;
  category: ConsoleApplicationNodeCatalogEntry['category'];
  field_contract: ConsoleApplicationNodeFieldContract;
  plugin: ConsolePluginNodeIdentity;
  disabled: boolean;
  disabledReason: string | null;
}

export type NodePickerOption =
  | BuiltinNodePickerOption
  | PluginContributionPickerOption;

export function toPluginContributionPickerOption(
  node: ConsoleApplicationNodeCatalogEntry
): PluginContributionPickerOption {
  if (node.source_kind !== 'plugin' || !node.plugin) {
    throw new Error(`Expected plugin catalog node: ${node.node_type}`);
  }

  const disabled =
    node.runtime_status === 'unavailable' || node.dependency_status !== 'ready';

  return {
    kind: 'plugin_contribution',
    label: node.title,
    description: node.description,
    category: node.category,
    field_contract: node.field_contract,
    plugin: node.plugin,
    disabled,
    disabledReason: disabled ? node.runtime_status_description : null
  };
}

export function buildNodePickerOptions(
  nodes: ConsoleApplicationNodeCatalogEntry[]
): NodePickerOption[] {
  return nodes.map((node) =>
    node.source_kind === 'builtin'
      ? toBuiltinNodePickerOption(node)
      : toPluginContributionPickerOption(node)
  );
}

export function getNodePickerOptionKey(option: NodePickerOption) {
  return option.kind === 'builtin'
    ? option.type
    : `${option.plugin.plugin_id}:${option.plugin.contribution_code}`;
}

export function getNodePickerOptionNodeType(option: NodePickerOption) {
  return option.kind === 'builtin' ? option.type : 'plugin_node';
}

export function getNodePickerOptionDescription(option: NodePickerOption) {
  return option.disabledReason ?? option.description;
}
