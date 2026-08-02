import { describe, expect, test } from 'vitest';

import { createNodeDocument } from '../lib/document/node-factory';
import {
  buildNodePickerOptions,
  createPluginNodeOutputs
} from '../lib/plugin-node-definitions';
import {
  createPluginCatalogNode,
  createPluginNodeIdentity
} from './fixtures/application-node-catalog';

const readyContribution = createPluginNodeIdentity({
  contribution_checksum: 'sha256:contribution',
  compiled_contribution_hash: 'sha256:compiled',
  output_schema_snapshot: {
    outputs: [
      {
        key: 'prompt_text',
        title: 'PromptText',
        valueType: 'string'
      }
    ]
  },
  output_schema: {
    outputs: [
      { key: 'legacy_output', title: 'Legacy Output', valueType: 'json' }
    ]
  }
});

describe('node contribution picker', () => {
  test('does not invent plugin outputs when the snapshot entry is incomplete', () => {
    expect(
      createPluginNodeOutputs({
        ...readyContribution,
        output_schema_snapshot: {
          outputs: [{ key: 'raw', title: 'Raw' }]
        }
      })
    ).toEqual([]);
  });

  test('writes contribution identity into the draft node document', () => {
    const option = buildNodePickerOptions([
      createPluginCatalogNode(readyContribution)
    ]).find(
      (entry) =>
        entry.kind === 'plugin_contribution' &&
        entry.plugin.contribution_code === 'openai_prompt'
    );

    if (!option) {
      throw new Error('Missing plugin contribution picker option');
    }

    const pluginNode = createNodeDocument(option, 'node-openai-prompt');

    expect(pluginNode).toMatchObject({
      id: 'node-openai-prompt',
      type: 'plugin_node',
      plugin_id: 'prompt_pack@0.1.0',
      plugin_version: '0.1.0',
      contribution_code: 'openai_prompt',
      node_shell: 'action',
      schema_version: '1flowbase.node-contribution/v2',
      plugin_unique_identifier: 'prompt_pack',
      package_id: 'prompt_pack@0.1.0',
      contribution_checksum: 'sha256:contribution',
      compiled_contribution_hash: 'sha256:compiled',
      output_schema_snapshot: {
        outputs: [
          {
            key: 'prompt_text',
            title: 'PromptText',
            valueType: 'string'
          }
        ]
      },
      outputs: [
        { key: 'prompt_text', title: 'PromptText', valueType: 'string' }
      ]
    });
  }, 20_000);
});
