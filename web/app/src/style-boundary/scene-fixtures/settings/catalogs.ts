import type { ConsoleMcpCatalog } from '@1flowbase/api-client';

import { i18nText } from '../../../shared/i18n/text';
import {
  modelProviderCatalogContract,
  primaryContractProviderEnabledModelIds
} from '../../../test/model-provider-contract-fixtures';

export const styleBoundaryProviderInstances = [
  {
    id: 'provider-openai-prod',
    installation_id: 'installation-openai-compatible',
    provider_code: 'openai_compatible',
    protocol: 'openai_responses',
    display_name: 'OpenAI Production',
    status: 'ready',
    config_json: {
      base_url: 'https://api.openai.com/v1',
      organization: 'workspace-prod'
    },
    enabled_model_ids: primaryContractProviderEnabledModelIds,
    catalog_refresh_status: 'succeeded',
    catalog_last_error_message: null,
    catalog_refreshed_at: '2026-04-18T16:01:00Z',
    model_count: primaryContractProviderEnabledModelIds.length
  }
];

export const styleBoundaryMcpCatalog = {
  instances: [
    {
      id: 'mcp-instance-record-1',
      workspace_id: 'workspace-1',
      instance_id: 'workspace_ops',
      name: 'Workspace Ops',
      description_short: 'Workspace MCP instance',
      status: 'enabled',
      default_entry_path: '/',
      created_by: 'user-1',
      updated_by: 'user-1',
      created_at: '2026-06-21T00:00:00Z',
      updated_at: '2026-06-21T00:00:00Z',
      llm_tool_registration: {
        prefix: 'workspace_ops',
        tools: [
          { operation: 'list', name: 'workspace_ops_mcp_list' },
          { operation: 'get', name: 'workspace_ops_mcp_get' },
          { operation: 'result', name: 'workspace_ops_mcp_result' },
          { operation: 'call', name: 'workspace_ops_mcp_call' }
        ]
      }
    }
  ],
  groups: [
    {
      id: 'mcp-group-1',
      instance_record_id: 'mcp-instance-record-1',
      path: '/ops',
      display_name: 'Operations',
      description_short: 'Operational tools',
      enabled: true,
      sort_order: 0
    }
  ],
  tools: [
    {
      id: 'mcp-tool-record-1',
      workspace_id: 'workspace-1',
      tool_id: 'runtime_profile_get',
      name: 'Runtime profile',
      short_description: 'Read runtime profile',
      full_description: 'Read the current system runtime profile.',
      execution_target: {
        kind: 'interface_wrapper' as const,
        interface_id: 'get_runtime_profile'
      },
      operation: 'GET /api/console/system/runtime-profile',
      parameter_schema: {
        type: 'object',
        properties: {
          query: {
            type: 'object',
            properties: { locale: { type: 'string' } },
            additionalProperties: false
          }
        },
        additionalProperties: false
      },
      result_schema: { type: 'object' },
      input_mapping: {
        type: 'object',
        properties: {
          query: {
            type: 'object',
            properties: { locale: { type: 'string' } },
            additionalProperties: false
          }
        },
        additionalProperties: false
      },
      output_mapping: { type: 'object' },
      permission_code: null,
      risk_level: 'low',
      des_id: 'Abc_1234',
      des_id_required: true,
      status: 'enabled',
      availability_status: 'available',
      availability_reason: null,
      revision: 1
    }
  ],
  bindings: [
    {
      id: 'mcp-binding-1',
      instance_record_id: 'mcp-instance-record-1',
      tool_record_id: 'mcp-tool-record-1',
      group_path: '/ops',
      tool_id: 'runtime_profile_get',
      display_alias: null,
      visible: true,
      sort_order: 0
    }
  ],
  discovery_policies: [
    {
      id: 'mcp-discovery-policy-1',
      workspace_id: 'workspace-1',
      instance_record_id: 'mcp-instance-record-1',
      instance_id: 'workspace_ops',
      list_default_limit: 20,
      list_max_depth: 3,
      list_regex_enabled: false,
      list_regex_max_length: 128,
      list_return_fields: ['path', 'name', 'risk_level']
    }
  ]
} satisfies ConsoleMcpCatalog;

export const styleBoundaryMcpInterfaceCapabilities = [
  {
    interface_id: 'get_runtime_profile',
    method: 'GET',
    path: '/api/console/system/runtime-profile',
    name: 'System runtime profile',
    short_description: 'Read current runtime profile',
    parameter_schema: {
      type: 'object',
      properties: {
        query: {
          type: 'object',
          properties: { locale: { type: 'string' } },
          additionalProperties: false
        }
      },
      additionalProperties: false
    },
    parameter_descriptors: [
      {
        name: 'query.locale',
        field_type: 'string',
        parameter_type: 'url' as const,
        description: 'Locale',
        required: false,
        schema: { type: 'string' }
      }
    ],
    result_schema: { type: 'object' },
    permission_code: null,
    security: [{ sessionCookie: [] }],
    risk_level: 'low',
    bindable: true,
    disabled_reason: null
  }
];

export const styleBoundaryNodeContributions = {
  nodes: [
    {
      source_kind: 'plugin',
      node_type: 'plugin_node',
      title: 'OpenAI Prompt',
      category: 'generation',
      runtime_status: 'ready',
      dependency_status: 'ready',
      field_contract: {
        config_fields: [],
        input_fields: [],
        output_fields: []
      },
      plugin: {
        installation_id: 'installation-1',
        provider_code: 'prompt_pack',
        plugin_id: 'prompt_pack@0.1.0',
        plugin_version: '0.1.0',
        contribution_code: 'openai_prompt',
        node_shell: 'action',
        plugin_unique_identifier: 'prompt_pack',
        package_id: 'prompt_pack@0.1.0',
        contribution_checksum: 'sha256:contribution',
        compiled_contribution_hash: 'sha256:compiled',
        category: 'generation',
        title: 'OpenAI Prompt',
        description: 'Generate prompt output',
        schema_version: '1flowbase.node-contribution/v2',
        output_schema_snapshot: {
          outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
        },
        experimental: false,
        icon: 'sparkles',
        schema_ui: {},
        output_schema: {
          outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
        },
        side_effect_policy: 'external_read',
        infra_contracts: [],
        required_auth: [],
        visibility: 'public',
        dependency_installation_kind: 'model_provider',
        dependency_plugin_version_range: '^0.1.0'
      }
    }
  ]
};

export const styleBoundaryApplicationRunRecord = {
  id: 'run-1',
  application_id: 'app-1',
  scope_id: 'workspace-1',
  run_mode: 'debug_flow_run',
  execution_stage: 'debug',
  invocation_source: 'debug',
  principal: {
    kind: 'user',
    id: 'user-1',
    display_name: 'Captain Root'
  },
  status: 'succeeded',
  target_node_id: null,
  title: 'Boundary run',
  expand_id: 'boundary-expand',
  external_user: null,
  authorized_account: 'root',
  api_key_id: null,
  api_key_name_snapshot: null,
  publication_version_id: null,
  external_conversation_id: null,
  external_trace_id: null,
  compatibility_mode: null,
  idempotency_key: null,
  total_tokens: 128,
  unique_node_count: 3,
  tool_callback_count: 0,
  started_at: '2026-05-10T09:00:00Z',
  finished_at: '2026-05-10T09:00:03Z',
  created_at: '2026-05-10T09:00:00Z',
  updated_at: '2026-05-10T09:00:03Z'
};

export function expandDottedBundle(bundle: Record<string, string>) {
  const expanded: Record<string, unknown> = {};

  for (const [dottedKey, value] of Object.entries(bundle)) {
    const segments = dottedKey.split('.');
    let current = expanded;

    for (const segment of segments.slice(0, -1)) {
      const next = current[segment];
      if (typeof next === 'object' && next !== null) {
        current = next as Record<string, unknown>;
        continue;
      }

      const created: Record<string, unknown> = {};
      current[segment] = created;
      current = created;
    }

    current[segments[segments.length - 1]!] = value;
  }

  return expanded;
}

export const styleBoundaryPluginI18nCatalog = Object.fromEntries(
  Object.entries(modelProviderCatalogContract.i18n_catalog).map(
    ([namespace, locales]) => [
      namespace,
      Object.fromEntries(
        Object.entries(locales as Record<string, Record<string, string>>).map(
          ([locale, bundle]) => [locale, expandDottedBundle(bundle)]
        )
      )
    ]
  )
);

export const styleBoundaryPluginFamiliesCatalog = {
  locale_meta: modelProviderCatalogContract.locale_meta,
  i18n_catalog: styleBoundaryPluginI18nCatalog,
  entries: modelProviderCatalogContract.entries.map((entry) => ({
    provider_code: entry.provider_code,
    plugin_type: 'model_provider',
    namespace: entry.namespace,
    label_key: entry.label_key,
    description_key: entry.description_key,
    provider_label_key: entry.label_key,
    protocol: entry.protocol,
    help_url: entry.help_url,
    default_base_url: entry.default_base_url,
    model_discovery_mode: entry.model_discovery_mode,
    current_installation_id: entry.installation_id,
    current_version: entry.plugin_version,
    current_local_artifact: {
      node_id: 'style-boundary-node',
      installation_id: entry.installation_id,
      local_version: entry.plugin_version,
      local_checksum: null,
      installed_path: `/tmp/1flowbase/plugins/${entry.provider_code}/${entry.plugin_version}`,
      artifact_status: 'ready',
      runtime_status: 'inactive',
      checked_at: '2026-04-20T10:00:00Z',
      last_error: null
    },
    latest_version: entry.plugin_version,
    has_update: false,
    installed_versions: [
      {
        installation_id: entry.installation_id,
        plugin_version: entry.plugin_version,
        source_kind: 'official_registry',
        trust_level: 'verified_official',
        desired_state: 'active',
        availability_status: 'available',
        local_artifact: {
          node_id: 'style-boundary-node',
          installation_id: entry.installation_id,
          local_version: entry.plugin_version,
          local_checksum: null,
          installed_path: `/tmp/1flowbase/plugins/${entry.provider_code}/${entry.plugin_version}`,
          artifact_status: 'ready',
          runtime_status: 'inactive',
          checked_at: '2026-04-20T10:00:00Z',
          last_error: null
        },
        created_at: '2026-04-20T10:00:00Z',
        is_current: true
      }
    ]
  }))
};

export const styleBoundaryOfficialPluginCatalog = {
  source_kind: 'official_registry',
  source_label: i18nText('appShell', 'auto.official_source'),
  registry_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/latest/download/official-registry.json',
  locale_meta: modelProviderCatalogContract.locale_meta,
  page: {
    limit: 20,
    next_cursor: null
  },
  entries: [
    {
      plugin_id: '1flowbase.openai_compatible',
      provider_code: 'openai_compatible',
      plugin_type: 'model_provider',
      display_name: 'OpenAI Compatible',
      description: 'Provider plugin for OpenAI-compatible APIs.',
      protocol: 'openai_responses',
      latest_version: '0.1.0',
      selected_artifact: {
        os: 'linux',
        arch: 'x64',
        libc: 'gnu',
        rust_target: 'x86_64-unknown-linux-gnu',
        download_url: 'https://example.com/openai-compatible.tar.gz',
        checksum: 'openai-compatible-checksum',
        signature_algorithm: null,
        signing_key_id: null
      },
      help_url:
        'https://github.com/taichuy/1flowbase-official-plugins/tree/main/models/openai_compatible',
      model_discovery_mode: 'hybrid',
      install_status: 'assigned'
    }
  ]
};
