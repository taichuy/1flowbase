import type {
  ConsoleApplicationNodeCatalog,
  ConsoleApplicationNodeCatalogEntry,
  ConsoleApplicationNodeFieldContract,
  ConsolePluginNodeIdentity
} from '@1flowbase/api-client';

export function createNodeFieldContract(
  overrides: Partial<ConsoleApplicationNodeFieldContract> = {}
): ConsoleApplicationNodeFieldContract {
  return {
    config_fields: [],
    input_fields: [],
    output_fields: [],
    ...overrides
  };
}

export function createWorkflowStartFieldContract(): ConsoleApplicationNodeFieldContract {
  return createNodeFieldContract({
    config_fields: [
      {
        key: 'config.input_fields[].inputType',
        required: true,
        value_types: ['string'],
        allowed_values: [
          'text',
          'paragraph',
          'select',
          'number',
          'checkbox',
          'file',
          'file_list',
          'url'
        ]
      },
      {
        key: 'config.input_fields[].source',
        required: true,
        value_types: ['string'],
        allowed_values: ['path', 'query', 'body', 'form']
      }
    ]
  });
}

export function createBuiltinCatalogNode(
  node_type: string,
  overrides: Partial<ConsoleApplicationNodeCatalogEntry> = {}
): ConsoleApplicationNodeCatalogEntry {
  return {
    source_kind: 'builtin',
    node_type,
    title: node_type,
    category: 'data',
    authoring_status: 'published',
    runtime_status: 'ready',
    dependency_status: 'not_applicable',
    field_contract: createNodeFieldContract(),
    plugin: null,
    ...overrides
  };
}

export function createPluginNodeIdentity(
  overrides: Partial<ConsolePluginNodeIdentity> = {}
): ConsolePluginNodeIdentity {
  return {
    installation_id: 'installation-1',
    provider_code: 'prompt_pack',
    plugin_unique_identifier: 'prompt_pack',
    package_id: 'prompt_pack@0.1.0',
    plugin_id: 'prompt_pack@0.1.0',
    plugin_version: '0.1.0',
    contribution_code: 'openai_prompt',
    node_shell: 'action',
    category: 'generation',
    title: 'OpenAI Prompt',
    description: 'Generate prompt output',
    schema_version: '1flowbase.node-contribution/v2',
    experimental: false,
    icon: 'sparkles',
    schema_ui: {},
    output_schema: {
      outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
    },
    contribution_checksum: 'sha256:openai-prompt',
    compiled_contribution_hash: 'sha256:compiled-openai-prompt',
    output_schema_snapshot: {
      outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
    },
    side_effect_policy: 'external_read',
    infra_contracts: [],
    required_auth: [],
    visibility: 'public',
    dependency_installation_kind: 'model_provider',
    dependency_plugin_version_range: '^0.1.0',
    ...overrides
  };
}

export function createPluginCatalogNode(
  plugin = createPluginNodeIdentity(),
  overrides: Partial<ConsoleApplicationNodeCatalogEntry> = {}
): ConsoleApplicationNodeCatalogEntry {
  return {
    source_kind: 'plugin',
    node_type: 'plugin_node',
    title: plugin.title,
    category: 'generation',
    authoring_status: 'published',
    runtime_status: 'ready',
    dependency_status: 'ready',
    field_contract: createNodeFieldContract(),
    plugin,
    ...overrides
  };
}

export function createApplicationNodeCatalog(
  nodes: ConsoleApplicationNodeCatalogEntry[]
): ConsoleApplicationNodeCatalog {
  return { nodes };
}
