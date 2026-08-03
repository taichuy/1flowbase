import {
  NODE_CONTRIBUTION_SCHEMA_VERSION,
  type FlowNodeDocument,
  type FlowPluginContributionOutputSchemaSnapshot,
  type FlowPluginContributionRef
} from '@1flowbase/flow-schema';

import type { ConsolePluginNodeIdentity } from '@1flowbase/api-client';
import './node-definitions/contracts';
import type {
  NodeDefinition,
  NodeDefinitionMeta
} from './node-definitions/types';
import { i18nText } from '../../../shared/i18n/text';
export {
  buildNodePickerOptions,
  getNodePickerOptionKey,
  getNodePickerOptionNodeType,
  toPluginContributionPickerOption,
  type NodePickerOption,
  type PluginContributionPickerOption
} from '../../flow-editor/authoring/plugin-node-picker';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getContributionOutputSchemaSnapshot(
  contribution: ConsolePluginNodeIdentity
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

export function toPluginContributionRef(
  contribution: ConsolePluginNodeIdentity
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
  contribution: ConsolePluginNodeIdentity
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
