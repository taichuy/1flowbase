import { vi } from 'vitest';
import {
  modelProviderCatalogContract,
  modelProviderCatalogEntries
} from '../../../../../test/model-provider-contract-fixtures';

const modelProviderApiFixtures = vi.hoisted(() => ({
  mainInstance: {
    provider_code: 'openai_compatible',
    auto_include_new_instances: true,
    revision: 1,
    model_routing_policies: [
      {
        model_id: 'gpt-4o-mini',
        distribution_rule: 'none',
        provider_instance_ids: ['provider-1']
      }
    ]
  },
  options: {
    locale_meta: {
      requested_locale: 'zh_Hans',
      resolved_locale: 'zh_Hans',
      user_preferred_locale: 'zh_Hans',
      accept_language: 'zh-Hans-CN,zh;q=0.9,en;q=0.8',
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    },
    i18n_catalog: {
      'plugin.openai_compatible': {
        zh_Hans: {
          'provider.label': 'OpenAI Compatible',
          'provider.description': 'OpenAI 协议兼容供应商'
        }
      }
    },
    providers: [
      {
        provider_code: 'openai_compatible',
        plugin_type: 'model_provider',
        namespace: 'plugin.openai_compatible',
        label_key: 'provider.label',
        description_key: 'provider.description',
        protocol: 'openai_responses',
        display_name: 'OpenAI Compatible',
        main_instance: {
          provider_code: 'openai_compatible',
          auto_include_new_instances: true,
          group_count: 2,
          model_count: 2
        },
        model_groups: [
          {
            model_id: 'gpt-4o-mini',
            distribution_rule: 'none',
            model: {
              model_id: 'gpt-4o-mini',
              display_name: 'GPT-4o Mini',
              source: 'runtime_catalog',
              supports_streaming: true,
              supports_tool_call: true,
              supports_multimodal: true,
              context_window: 128000,
              max_output_tokens: 16384,
              provider_metadata: {}
            },
            targets: [
              {
                source_instance_id: 'provider-openai-prod',
                source_instance_display_name: 'OpenAI Production',
                model: {
                  model_id: 'gpt-4o-mini',
                  display_name: 'GPT-4o Mini',
                  source: 'runtime_catalog',
                  supports_streaming: true,
                  supports_tool_call: true,
                  supports_multimodal: true,
                  context_window: 128000,
                  max_output_tokens: 16384,
                  provider_metadata: {}
                }
              }
            ]
          }
        ]
      }
    ]
  }
}));

export const modelProviderOptionsContract = modelProviderApiFixtures.options;

vi.mock('@1flowbase/api-client', () => ({
  fetchConsoleApiDocsCatalog: vi.fn().mockResolvedValue({ categories: [] }),
  fetchConsoleApiDocsCategoryOperations: vi.fn().mockResolvedValue({
    id: 'console',
    operations: []
  }),
  fetchConsoleApiOperationSpec: vi.fn().mockResolvedValue({ openapi: '3.1.0' }),
  listConsoleMembers: vi.fn().mockResolvedValue([]),
  createConsoleMember: vi.fn().mockResolvedValue({ id: 'member-1' }),
  updateConsoleMember: vi.fn().mockResolvedValue({ id: 'member-1' }),
  disableConsoleMember: vi.fn().mockResolvedValue(undefined),
  enableConsoleMember: vi.fn().mockResolvedValue(undefined),
  deleteConsoleMember: vi.fn().mockResolvedValue(undefined),
  resetConsoleMemberPassword: vi.fn().mockResolvedValue(undefined),
  changeConsolePassword: vi.fn().mockResolvedValue(undefined),
  replaceConsoleMemberRoles: vi.fn().mockResolvedValue(undefined),
  listConsolePermissions: vi.fn().mockResolvedValue([]),
  listConsoleRoles: vi.fn().mockResolvedValue([]),
  createConsoleRole: vi.fn().mockResolvedValue({ code: 'member' }),
  updateConsoleRole: vi.fn().mockResolvedValue(undefined),
  deleteConsoleRole: vi.fn().mockResolvedValue(undefined),
  fetchConsoleRolePermissions: vi.fn().mockResolvedValue({
    role_code: 'member',
    permission_codes: []
  }),
  replaceConsoleRolePermissions: vi.fn().mockResolvedValue(undefined),
  listConsoleModelProviderCatalog: vi
    .fn()
    .mockResolvedValue(modelProviderCatalogContract),
  listConsoleModelProviderInstances: vi.fn().mockResolvedValue([]),
  listConsoleSettingsModelProviderOptions: vi
    .fn()
    .mockResolvedValue(modelProviderApiFixtures.options),
  getConsoleModelProviderMainInstance: vi
    .fn()
    .mockResolvedValue(modelProviderApiFixtures.mainInstance),
  getConsoleModelProviderModels: vi.fn().mockResolvedValue({
    provider_instance_id: 'provider-1',
    models: []
  }),
  previewConsoleModelProviderModels: vi.fn().mockResolvedValue({
    models: [],
    preview_token: 'preview-1',
    expires_at: '2026-04-22T12:00:00Z'
  }),
  createConsoleModelProviderInstance: vi.fn().mockResolvedValue({
    id: 'provider-1'
  }),
  updateConsoleModelProviderInstance: vi.fn().mockResolvedValue(undefined),
  updateConsoleModelProviderMainInstance: vi
    .fn()
    .mockResolvedValue(modelProviderApiFixtures.mainInstance),
  validateConsoleModelProviderInstance: vi.fn().mockResolvedValue({
    instance: {
      id: 'provider-1',
      installation_id: 'installation-1',
      provider_code: 'openai_compatible',
      protocol: 'openai_compatible',
      display_name: 'OpenAI Production',
      status: 'ready',
      included_in_main: true,
      config_json: {
        base_url: 'https://api.openai.com/v1'
      },
      configured_models: [
        {
          model_id: 'gpt-4o-mini',
          enabled: true,
          context_window_override_tokens: null
        },
        {
          model_id: 'gpt-4o',
          enabled: false,
          context_window_override_tokens: null
        }
      ],
      enabled_model_ids: ['gpt-4o-mini'],
      catalog_refresh_status: 'ready',
      catalog_last_error_message: null,
      catalog_refreshed_at: '2026-04-18T10:01:00Z',
      model_count: 2
    },
    output: {}
  }),
  refreshConsoleModelProviderModels: vi.fn().mockResolvedValue({
    provider_instance_id: 'provider-1',
    models: []
  }),
  revealConsoleModelProviderSecret: vi.fn().mockResolvedValue({
    key: 'api_key',
    value: 'super-secret'
  }),
  deleteConsoleModelProviderInstance: vi.fn().mockResolvedValue(undefined),
  listConsolePluginFamilies: vi.fn().mockResolvedValue({
    locale_meta: {
      requested_locale: null,
      resolved_locale: 'zh_Hans',
      user_preferred_locale: null,
      accept_language: null,
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    },
    i18n_catalog: {},
    entries: []
  }),
  listConsoleOfficialPluginCatalog: vi.fn().mockResolvedValue({
    source_kind: 'official_registry',
    source_label: '官方源',
    registry_url: 'https://official.example.com/official-registry.json',
    locale_meta: {
      requested_locale: null,
      resolved_locale: 'zh_Hans',
      user_preferred_locale: null,
      accept_language: null,
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    },
    page: {
      limit: 20,
      next_cursor: null
    },
    entries: []
  }),
  installConsoleOfficialPlugin: vi.fn().mockResolvedValue({
    installation: { id: 'installation-1' }
  }),
  fetchConsoleFileStorages: vi.fn().mockResolvedValue([]),
  createConsoleFileStorage: vi.fn().mockResolvedValue({ id: 'storage-1' }),
  fetchConsoleFileTables: vi.fn().mockResolvedValue([]),
  createConsoleFileTable: vi.fn().mockResolvedValue({ id: 'table-1' }),
  updateConsoleFileTableBinding: vi.fn().mockResolvedValue({ id: 'table-1' }),
  uploadConsolePluginPackage: vi.fn().mockResolvedValue({
    installation: { id: 'installation-upload' }
  }),
  upgradeConsolePluginFamilyLatest: vi.fn().mockResolvedValue({
    id: 'task-upgrade'
  }),
  switchConsolePluginFamilyVersion: vi.fn().mockResolvedValue({
    id: 'task-switch'
  }),
  getConsolePluginTask: vi.fn().mockResolvedValue({
    id: 'task-1'
  }),
  listConsoleHostInfrastructureProviders: vi.fn().mockResolvedValue([]),
  getConsoleHostInfrastructureMemoryOverview: vi.fn().mockResolvedValue({
    can_manage: true,
    contracts: []
  }),
  getConsoleHostInfrastructureMemoryStatsOverview: vi.fn().mockResolvedValue({
    inspection_path: [],
    contracts: [],
    entry_count: 0,
    sensitive_entry_count: 0,
    total_value_size_bytes: 0
  }),
  getConsoleHostInfrastructureMemoryStats: vi.fn().mockResolvedValue({
    contract_code: 'session-store',
    label: 'Sessions',
    provider_code: 'local',
    supported: true,
    inspection_path: [],
    entry_count: 0,
    sensitive_entry_count: 0,
    total_value_size_bytes: 0
  }),
  listConsoleHostInfrastructureMemoryEntries: vi.fn().mockResolvedValue({
    contract_code: 'session-store',
    label: 'Sessions',
    provider_code: 'local',
    capabilities: {
      list_entries: true,
      list_tree: true,
      search_entries: true,
      reveal_value: true,
      default_page_size: 50,
      max_page_size: 200,
      default_byte_limit: 65536,
      max_byte_limit: 262144,
      default_preview_size_bytes: 8192,
      max_full_value_size_bytes: 262144
    },
    supported: true,
    inspection_path: [],
    entries: [],
    next_cursor: null,
    limit: 50,
    byte_limit: 65536,
    emitted_bytes: 0,
    truncated_by_byte_limit: false
  }),
  listConsoleHostInfrastructureMemoryTree: vi.fn().mockResolvedValue({
    contract_code: 'session-store',
    label: 'Sessions',
    provider_code: 'local',
    capabilities: {
      list_entries: true,
      list_tree: true,
      search_entries: true,
      reveal_value: true,
      default_page_size: 50,
      max_page_size: 200,
      default_byte_limit: 65536,
      max_byte_limit: 262144,
      default_preview_size_bytes: 8192,
      max_full_value_size_bytes: 262144
    },
    supported: true,
    inspection_path: [],
    nodes: [],
    next_cursor: null,
    limit: 50,
    byte_limit: 65536,
    emitted_bytes: 0,
    truncated_by_byte_limit: false
  }),
  searchConsoleHostInfrastructureMemoryEntries: vi.fn().mockResolvedValue({
    contract_code: 'session-store',
    label: 'Sessions',
    provider_code: 'local',
    capabilities: {
      list_entries: true,
      list_tree: true,
      search_entries: true,
      reveal_value: true,
      default_page_size: 50,
      max_page_size: 200,
      default_byte_limit: 65536,
      max_byte_limit: 262144,
      default_preview_size_bytes: 8192,
      max_full_value_size_bytes: 262144
    },
    supported: true,
    inspection_path: [],
    entries: [],
    next_cursor: null,
    limit: 50,
    byte_limit: 65536,
    emitted_bytes: 0,
    truncated_by_byte_limit: false
  }),
  revealConsoleHostInfrastructureMemoryEntry: vi.fn().mockResolvedValue({
    metadata: {
      contract_code: 'session-store',
      group_code: 'sessions',
      entry_ref: 'session:1',
      key: 'session:1',
      inspection_path: ['sessions', 'session:1'],
      entry_kind: 'session',
      status: 'active',
      owner: null,
      value_size_bytes: 12,
      metadata_size_bytes: 2,
      ttl_seconds: null,
      created_at_unix: null,
      expires_at_unix: null,
      sensitive: true,
      metadata: {}
    },
    reveal_mode: 'preview',
    value_state: 'available',
    value: { ok: true },
    value_preview: null,
    preview_size_bytes: 12,
    full_value_size_bytes: 12
  }),
  getConsoleHostInfrastructureCacheOverview: vi.fn().mockResolvedValue({
    provider_code: 'local',
    can_manage: true,
    capabilities: {
      list_domains: true,
      list_entries: true,
      reveal_value: true,
      clear_entry: true,
      clear_domain: true
    },
    domains: []
  }),
  listConsoleHostInfrastructureCacheEntries: vi.fn().mockResolvedValue({
    domain_code: 'application-logs',
    capabilities: {
      list_domains: true,
      list_entries: true,
      reveal_value: true,
      clear_entry: true,
      clear_domain: true
    },
    entries: []
  }),
  revealConsoleHostInfrastructureCacheEntry: vi.fn().mockResolvedValue({
    metadata: {
      domain_code: 'application-logs',
      key: 'application-logs:run:1',
      value_size_bytes: 12,
      ttl_seconds: null,
      created_at_unix: null,
      expires_at_unix: null
    },
    value: { ok: true }
  }),
  clearConsoleHostInfrastructureCacheEntry: vi.fn().mockResolvedValue({
    cleared: true
  }),
  clearConsoleHostInfrastructureCacheDomain: vi.fn().mockResolvedValue({
    cleared_count: 1
  }),
  saveConsoleHostInfrastructureProviderConfig: vi.fn().mockResolvedValue({
    restart_required: true,
    installation_desired_state: 'pending_restart',
    provider_config_status: 'pending_restart'
  }),
  fetchConsoleSystemRuntimeProfile: vi.fn().mockResolvedValue({
    topology: { relationship: 'same_host' },
    hosts: []
  })
}));

export { modelProviderCatalogContract, modelProviderCatalogEntries };
