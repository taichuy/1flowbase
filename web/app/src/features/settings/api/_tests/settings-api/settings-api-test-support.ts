import { vi } from 'vitest';
import { modelProviderCatalogContract } from '../../../../../test/model-provider-contract-fixtures';

const modelProviderApiFixtures = vi.hoisted(() => ({
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
                routing_enabled: true,
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
  fetchConsoleApiDocsCategoryOperations: vi.fn(),
  fetchConsoleApiOperationSpec: vi.fn(),
  listConsoleMembers: vi.fn().mockResolvedValue([]),
  createConsoleMember: vi.fn(),
  updateConsoleMember: vi.fn(),
  disableConsoleMember: vi.fn(),
  enableConsoleMember: vi.fn(),
  deleteConsoleMember: vi.fn(),
  resetConsoleMemberPassword: vi.fn(),
  changeConsolePassword: vi.fn(),
  replaceConsoleMemberRoles: vi.fn(),
  listConsolePricingRules: vi.fn().mockResolvedValue([]),
  createConsolePricingRule: vi.fn().mockResolvedValue({ id: 'pricing-rule-1' }),
  updateConsolePricingRule: vi.fn().mockResolvedValue({ id: 'pricing-rule-1' }),
  deleteConsolePricingRule: vi.fn().mockResolvedValue({ deleted: true }),
  getConsolePricingCatalog: vi.fn().mockResolvedValue({
    schema_version: '1flowbase.model-pricing-page/v1',
    catalog_version: 'test',
    currency_code: 'USD',
    items: [],
    total_count: 0,
    page: 1,
    page_size: 20
  }),
  importConsolePricingCatalog: vi
    .fn()
    .mockResolvedValue({ inserted: 0, skipped: 0, updated: 0, deleted: 0 }),
  listConsoleCreditAccounts: vi.fn().mockResolvedValue([]),
  listConsoleCreditLedger: vi.fn().mockResolvedValue([]),
  executeConsoleCreditCommand: vi
    .fn()
    .mockResolvedValue({ id: 'credit-ledger-1' }),
  listConsolePermissions: vi.fn().mockResolvedValue([]),
  listConsoleRoles: vi.fn().mockResolvedValue([]),
  createConsoleRole: vi.fn(),
  updateConsoleRole: vi.fn(),
  deleteConsoleRole: vi.fn(),
  fetchConsoleRolePermissions: vi.fn(),
  replaceConsoleRolePermissions: vi.fn(),
  listConsoleModelProviderCatalog: vi
    .fn()
    .mockResolvedValue(modelProviderCatalogContract),
  listConsoleModelProviderInstances: vi.fn().mockResolvedValue([]),
  listConsoleSettingsModelProviderOptions: vi
    .fn()
    .mockResolvedValue(modelProviderApiFixtures.options),
  getConsoleModelProviderMainInstance: vi.fn(),
  getConsoleModelProviderModels: vi.fn(),
  previewConsoleModelProviderModels: vi.fn(),
  createConsoleModelProviderInstance: vi.fn(),
  updateConsoleModelProviderInstance: vi.fn(),
  updateConsoleModelProviderMainInstance: vi.fn(),
  authenticateConsoleModelProviderInstance: vi.fn(),
  validateConsoleModelProviderInstance: vi.fn(),
  refreshConsoleModelProviderModels: vi.fn(),
  revealConsoleModelProviderSecret: vi.fn(),
  deleteConsoleModelProviderInstance: vi.fn(),
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
  createConsoleFileStorage: vi.fn(),
  fetchConsoleFileTables: vi.fn().mockResolvedValue([]),
  createConsoleFileTable: vi.fn(),
  updateConsoleFileTableBinding: vi.fn(),
  uploadConsolePluginPackage: vi.fn(),
  upgradeConsolePluginFamilyLatest: vi.fn().mockResolvedValue({
    id: 'task-upgrade'
  }),
  switchConsolePluginFamilyVersion: vi.fn().mockResolvedValue({
    id: 'task-switch'
  }),
  getConsolePluginTask: vi.fn(),
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
  listConsoleHostInfrastructureMemoryEntries: vi.fn(),
  listConsoleHostInfrastructureMemoryTree: vi.fn(),
  searchConsoleHostInfrastructureMemoryEntries: vi.fn(),
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
  listConsoleHostInfrastructureCacheEntries: vi.fn(),
  revealConsoleHostInfrastructureCacheEntry: vi.fn(),
  clearConsoleHostInfrastructureCacheEntry: vi.fn(),
  clearConsoleHostInfrastructureCacheDomain: vi.fn(),
  fetchConsoleSystemRuntimeProfile: vi.fn().mockResolvedValue({
    topology: { relationship: 'same_host' },
    hosts: []
  }),
  fetchConsoleSystemRuntimeProcesses: vi.fn().mockResolvedValue({
    process_total: 0,
    processes: []
  }),
  terminateConsoleSystemProcess: vi.fn()
}));

export { modelProviderCatalogContract };
