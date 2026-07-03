import {
  NODE_CONTRIBUTION_SCHEMA_VERSION,
  type BuiltinFlowNodeType,
  type FlowNodeDocument,
  type FlowPluginContributionOutputSchemaSnapshot,
  type FlowPluginContributionRef
} from '@1flowbase/flow-schema';

import type { AgentFlowNodeContributionEntry } from '../api/node-contributions';
import type {
  NodeDefinition,
  NodeDefinitionMeta
} from './node-definitions/types';
import {
  builtinNodeRuntimeContractTypes,
  getBuiltinNodeRuntimeContract
} from './node-definitions/contracts';
import { i18nText } from '../../../shared/i18n/text';

export interface BuiltinNodePickerOption {
  kind: 'builtin';
  type: BuiltinFlowNodeType;
  label: string;
  description: string;
  category: string | null;
  inputKeys: string[];
  outputKeys: string[];
}

export interface PluginContributionPickerOption {
  kind: 'plugin_contribution';
  label: string;
  contribution: AgentFlowNodeContributionEntry;
  disabled: boolean;
  disabledReason: string | null;
}

export type NodePickerOption =
  | BuiltinNodePickerOption
  | PluginContributionPickerOption;

// These nodes are incomplete and not available to users yet.
const UNRELEASED_BUILTIN_NODE_PICKER_TYPES = new Set<BuiltinFlowNodeType>([
  'human_input',
  'iteration',
  'loop',
  'parameter_extractor',
  'tool'
]);

const HIDDEN_BUILTIN_NODE_PICKER_TYPES = new Set<BuiltinFlowNodeType>([
  'workflow_start',
  'workflow_end',
  ...UNRELEASED_BUILTIN_NODE_PICKER_TYPES
]);

function buildBuiltinNodePickerOptions(
  nodeTypes: readonly BuiltinFlowNodeType[]
): BuiltinNodePickerOption[] {
  return nodeTypes.map((nodeType) => {
    const contract = getBuiltinNodeRuntimeContract(nodeType);

    if (!contract) {
      throw new Error(`Missing runtime contract for node picker: ${nodeType}`);
    }

    return {
      kind: 'builtin',
      type: nodeType,
      label: contract.meta.title,
      description:
        contract.defaults.description ?? contract.card.description ?? '',
      category: contract.card.category ?? null,
      inputKeys: contract.ports.inputs.map((port) => port.key),
      outputKeys: contract.ports.outputs.map((port) => port.key)
    };
  });
}

export const BUILTIN_NODE_PICKER_OPTIONS: BuiltinNodePickerOption[] =
  buildBuiltinNodePickerOptions(
    builtinNodeRuntimeContractTypes
      .filter(
        (nodeType): nodeType is BuiltinFlowNodeType =>
          nodeType !== 'plugin_node'
      )
      .filter((nodeType) => !HIDDEN_BUILTIN_NODE_PICKER_TYPES.has(nodeType))
  );

const GENERAL_EXECUTION_NODE_PICKER_TYPES = builtinNodeRuntimeContractTypes.filter(
  (nodeType): nodeType is BuiltinFlowNodeType =>
    nodeType !== 'plugin_node' &&
    nodeType !== 'start' &&
    nodeType !== 'answer' &&
    nodeType !== 'workflow_start' &&
    nodeType !== 'workflow_end' &&
    !UNRELEASED_BUILTIN_NODE_PICKER_TYPES.has(nodeType)
);

export const WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS: BuiltinNodePickerOption[] =
  buildBuiltinNodePickerOptions([
    'workflow_start',
    'workflow_end',
    ...GENERAL_EXECUTION_NODE_PICKER_TYPES
  ]);

const DEPENDENCY_STATUS_LABELS: Record<string, string> = {
  missing_plugin: i18nText('agentFlow', 'auto.dependency_missing_plugin'),
  version_mismatch: i18nText('agentFlow', 'auto.dependency_version_mismatch'),
  disabled_plugin: i18nText('agentFlow', 'auto.dependency_plugin_not_ready')
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getContributionOutputSchemaSnapshot(
  contribution: AgentFlowNodeContributionEntry
): FlowPluginContributionOutputSchemaSnapshot {
  return isRecord(contribution.output_schema_snapshot)
    ? contribution.output_schema_snapshot
    : {};
}

export const pluginNodeDefinition: NodeDefinition = {
  label: i18nText('agentFlow', 'auto.plugin_node_label'),
  summary: i18nText('agentFlow', 'auto.plugin_node_definition_summary'),
  helpHref: null,
  sections: [
    {
      key: 'basics',
      title: i18nText('agentFlow', 'auto.basic_information'),
      fields: []
    },
    {
      key: 'outputs',
      title: i18nText('agentFlow', 'auto.outputs'),
      fields: []
    }
  ]
};

export const pluginNodeDefinitionMeta: NodeDefinitionMeta = {
  summary: i18nText('agentFlow', 'auto.plugin_node_meta_summary'),
  helpHref: null
};

function toPluginContributionPickerOption(
  contribution: AgentFlowNodeContributionEntry
): PluginContributionPickerOption {
  return {
    kind: 'plugin_contribution' as const,
    label: contribution.title,
    contribution,
    disabled: contribution.dependency_status !== 'ready',
    disabledReason:
      contribution.dependency_status === 'ready'
        ? null
        : (DEPENDENCY_STATUS_LABELS[contribution.dependency_status] ??
          i18nText('agentFlow', 'auto.plugin_node_unavailable'))
  };
}

export function buildNodePickerOptions(
  contributions: AgentFlowNodeContributionEntry[]
): NodePickerOption[] {
  return [
    ...BUILTIN_NODE_PICKER_OPTIONS,
    ...contributions.map(toPluginContributionPickerOption)
  ];
}

export function buildWorkflowNodePickerOptions(
  contributions: AgentFlowNodeContributionEntry[]
): NodePickerOption[] {
  return [
    ...WORKFLOW_BUILTIN_NODE_PICKER_OPTIONS,
    ...contributions.map(toPluginContributionPickerOption)
  ];
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

export function toPluginContributionRef(
  contribution: AgentFlowNodeContributionEntry
): FlowPluginContributionRef {
  return {
    plugin_id: contribution.plugin_id,
    plugin_version: contribution.plugin_version,
    contribution_code: contribution.contribution_code,
    node_shell: contribution.node_shell,
    schema_version: contribution.schema_version,
    plugin_unique_identifier: contribution.plugin_unique_identifier,
    package_id: contribution.package_id,
    contribution_checksum: contribution.contribution_checksum,
    compiled_contribution_hash: contribution.compiled_contribution_hash,
    output_schema_snapshot: getContributionOutputSchemaSnapshot(contribution)
  };
}

function hasContributionOutput(
  entry: unknown
): entry is FlowPluginContributionOutputSchemaSnapshot {
  return isRecord(entry) && Array.isArray(entry.outputs);
}

export function hasPluginContributionRef(
  node: Partial<FlowPluginContributionRef>
): node is FlowPluginContributionRef {
  if (node.schema_version !== NODE_CONTRIBUTION_SCHEMA_VERSION) {
    return false;
  }

  return (
    [
      node.plugin_id,
      node.plugin_version,
      node.contribution_code,
      node.node_shell,
      node.plugin_unique_identifier,
      node.package_id,
      node.contribution_checksum,
      node.compiled_contribution_hash
    ].every((value) => typeof value === 'string' && value.trim().length > 0) &&
    hasContributionOutput(node.output_schema_snapshot)
  );
}

export function createPluginNodeOutputs(
  contribution: AgentFlowNodeContributionEntry
): FlowNodeDocument['outputs'] {
  const schemaOutputs =
    getContributionOutputSchemaSnapshot(contribution).outputs;

  if (!Array.isArray(schemaOutputs)) {
    return [];
  }

  const outputs = schemaOutputs
    .map((entry) => {
      if (!entry || typeof entry !== 'object') {
        return null;
      }

      const key =
        typeof entry.key === 'string' && entry.key.trim().length > 0
          ? entry.key
          : null;
      const title =
        typeof entry.title === 'string' && entry.title.trim().length > 0
          ? entry.title
          : null;
      const valueType =
        typeof entry.valueType === 'string' && entry.valueType.trim().length > 0
          ? entry.valueType
          : null;

      if (!key || !title || !valueType) {
        return null;
      }

      return {
        key,
        title,
        valueType
      };
    })
    .filter(
      (entry): entry is FlowNodeDocument['outputs'][number] => entry !== null
    );

  return outputs;
}
