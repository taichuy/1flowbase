import { afterEach, describe, expect, expectTypeOf, test, vi } from 'vitest';
import {
  modelProviderCatalogContract,
  modelProviderCatalogEntries
} from '../../../../../test/model-provider-contract-fixtures';
import { modelProviderOptionsContract } from './settings-api-test-support';

test('D5-F02 does not expose a feature-level universal extension upload API', () => {
  type SettingsExtensionsApi = typeof import('../../extensions/extensions');
  // @ts-expect-error Universal upload is intentionally absent from the feature API.
  expectTypeOf<SettingsExtensionsApi['uploadSettingsExtension']>().toBeNever();
});

import {
  fetchConsoleApiDocsCatalog,
  listConsoleMembers,
  listConsolePermissions,
  listConsoleRoles,
  listConsoleModelProviderCatalog,
  listConsoleModelProviderInstances,
  listConsoleSettingsModelProviderOptions,
  listConsolePluginFamilies,
  listConsoleOfficialPluginCatalog,
  installConsoleOfficialPlugin,
  fetchConsoleFileStorages,
  fetchConsoleFileTables,
  upgradeConsolePluginFamilyLatest,
  switchConsolePluginFamilyVersion,
  getConsoleHostInfrastructureMemoryOverview,
  getConsoleHostInfrastructureMemoryStatsOverview,
  getConsoleHostInfrastructureMemoryStats,
  revealConsoleHostInfrastructureMemoryEntry,
  getConsoleHostInfrastructureCacheOverview
} from '@1flowbase/api-client';
import type {
  ConsoleModelProviderInstance,
  ConsolePluginFamilyEntry
} from '@1flowbase/api-client';

expectTypeOf<ConsoleModelProviderInstance>()
  .toHaveProperty('included_in_main')
  .toEqualTypeOf<boolean>();
expectTypeOf<ConsoleModelProviderInstance>()
  .toHaveProperty('enabled_model_ids')
  .toEqualTypeOf<string[]>();
expectTypeOf<ConsoleModelProviderInstance>().not.toHaveProperty(
  'validation_model_id'
);

import {
  settingsApiDocsCatalogQueryKey,
  settingsApiDocsCategoryOperationsQueryKey,
  settingsApiDocsOperationSpecQueryKey,
  fetchSettingsApiDocsCatalog
} from '../../api-docs';
import {
  settingsMembersQueryKey,
  fetchSettingsMembers
} from '../../identity-access/members';
import {
  settingsPermissionsQueryKey,
  fetchSettingsPermissions
} from '../../identity-access/permissions';
import {
  settingsRolesQueryKey,
  settingsRolePermissionsQueryKey,
  fetchSettingsRoles
} from '../../identity-access/roles';
import {
  settingsModelProviderCatalogQueryKey,
  settingsModelProviderInstancesQueryKey,
  settingsModelProviderOptionsQueryKey,
  settingsModelProviderModelsQueryKey,
  fetchSettingsModelProviderCatalog,
  fetchSettingsModelProviderInstances,
  fetchSettingsModelProviderOptions
} from '../../model-providers';
import type { SettingsModelProviderInstance } from '../../model-providers';
import {
  settingsPluginFamiliesQueryKey,
  settingsOfficialPluginsQueryKey,
  fetchSettingsPluginFamilies,
  fetchSettingsOfficialPluginCatalog,
  installSettingsOfficialPlugin,
  upgradeSettingsPluginFamilyLatest,
  switchSettingsPluginFamilyVersion
} from '../../extensions/plugins';
import {
  settingsFileStoragesQueryKey,
  settingsFileTablesQueryKey,
  fetchSettingsFileStorages,
  fetchSettingsFileTables
} from '../../file-management';
import {
  fetchSettingsHostInfrastructureCacheOverview,
  fetchSettingsHostInfrastructureMemoryOverview,
  fetchSettingsHostInfrastructureMemoryStatsOverview,
  fetchSettingsHostInfrastructureMemoryStats,
  revealSettingsHostInfrastructureMemoryEntry,
  settingsHostInfrastructureCacheEntriesQueryKey,
  settingsHostInfrastructureCacheOverviewQueryKey,
  settingsHostInfrastructureMemoryEntriesQueryKey,
  settingsHostInfrastructureMemoryOverviewQueryKey,
  settingsHostInfrastructureMemoryStatsOverviewQueryKey,
  settingsHostInfrastructureMemoryStatsQueryKey,
  settingsHostInfrastructureMemorySearchQueryKey,
  settingsHostInfrastructureMemoryTreeQueryKey
} from '../../host-infrastructure';

const pluginFamilyEntry = {
  provider_code: 'openai_compatible',
  plugin_type: 'model_provider',
  namespace: 'plugin.openai_compatible',
  label_key: 'plugin.label',
  description_key: 'plugin.description',
  provider_label_key: 'provider.label',
  protocol: 'openai_compatible',
  help_url: 'https://platform.openai.com/docs/api-reference',
  default_base_url: 'https://api.openai.com/v1',
  model_discovery_mode: 'hybrid',
  current_installation_id: 'installation-1',
  current_version: '0.3.7',
  installation_status: 'assigned',
  current_local_artifact: {
    node_id: 'test-node',
    installation_id: 'installation-1',
    local_version: '0.3.7',
    local_checksum: null,
    installed_path: '/tmp/plugins/openai_compatible/0.3.7',
    artifact_status: 'ready',
    runtime_status: 'inactive',
    checked_at: '2026-04-20T10:00:00Z',
    last_error: null
  },
  latest_version: '0.3.7',
  has_update: false,
  installed_versions: []
} satisfies ConsolePluginFamilyEntry;

afterEach(() => {
  vi.clearAllMocks();
});

describe('settings API query and adapter behavior', () => {
  test('keeps API docs query keys and catalog callback', async () => {
    expect(settingsApiDocsCatalogQueryKey).toEqual([
      'settings',
      'docs',
      'catalog'
    ]);
    expect(settingsApiDocsCategoryOperationsQueryKey('console')).toEqual([
      'settings',
      'docs',
      'category',
      'console',
      'operations'
    ]);
    expect(settingsApiDocsOperationSpecQueryKey('op-1')).toEqual([
      'settings',
      'docs',
      'operation',
      'op-1',
      'openapi'
    ]);

    await fetchSettingsApiDocsCatalog();

    expect(fetchConsoleApiDocsCatalog).toHaveBeenCalledTimes(1);
  });

  test('keeps identity query keys and member department argument position', async () => {
    expect(settingsMembersQueryKey).toEqual(['settings', 'members']);
    expect(settingsPermissionsQueryKey).toEqual(['settings', 'permissions']);
    expect(settingsRolesQueryKey).toEqual(['settings', 'roles']);
    expect(settingsRolePermissionsQueryKey('member')).toEqual([
      'settings',
      'roles',
      'member',
      'permissions'
    ]);

    await fetchSettingsMembers('department-1');

    await fetchSettingsPermissions();
    await fetchSettingsRoles();

    expect(listConsoleMembers).toHaveBeenCalledWith(undefined, 'department-1');

    expect(listConsolePermissions).toHaveBeenCalledTimes(1);
    expect(listConsoleRoles).toHaveBeenCalledTimes(1);
  });

  test('keeps model provider query keys and catalog projections', async () => {
    const instance = {
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
    } satisfies SettingsModelProviderInstance;

    expect(settingsModelProviderCatalogQueryKey).toEqual([
      'settings',
      'model-providers',
      'catalog'
    ]);
    expect(settingsModelProviderInstancesQueryKey).toEqual([
      'settings',
      'model-providers',
      'instances'
    ]);
    expect(settingsModelProviderOptionsQueryKey).toEqual([
      'settings',
      'model-providers',
      'options'
    ]);
    expect(settingsModelProviderModelsQueryKey('provider-1')).toEqual([
      'settings',
      'model-providers',
      'models',
      'provider-1'
    ]);

    await expect(fetchSettingsModelProviderCatalog()).resolves.toEqual(
      modelProviderCatalogEntries
    );
    vi.mocked(listConsoleModelProviderInstances).mockResolvedValueOnce([
      instance
    ]);
    const fetchedInstances = await fetchSettingsModelProviderInstances();
    expect(fetchedInstances).toHaveLength(1);
    expect(fetchedInstances[0]).toEqual(
      expect.objectContaining({
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
        enabled_model_ids: ['gpt-4o-mini']
      })
    );
    expect(fetchedInstances[0]).not.toHaveProperty('validation_model_id');
    expect(fetchedInstances[0]).not.toHaveProperty('last_validated_at');
    expect(fetchedInstances[0]).not.toHaveProperty('last_validation_status');
    expect(fetchedInstances[0]).not.toHaveProperty('last_validation_message');
    await expect(fetchSettingsModelProviderOptions()).resolves.toEqual(
      modelProviderOptionsContract
    );

    expect(listConsoleModelProviderCatalog).toHaveBeenCalledTimes(1);
    expect(listConsoleModelProviderInstances).toHaveBeenCalledTimes(1);
    expect(listConsoleSettingsModelProviderOptions).toHaveBeenCalledTimes(1);

    expect(modelProviderOptionsContract.providers[0]).toEqual(
      expect.objectContaining({
        provider_code: 'openai_compatible',
        plugin_type: 'model_provider',
        namespace: 'plugin.openai_compatible',
        label_key: 'provider.label',
        description_key: 'provider.description',
        main_instance: expect.objectContaining({
          provider_code: 'openai_compatible',
          auto_include_new_instances: true
        }),
        model_groups: [
          expect.objectContaining({
            model_id: 'gpt-4o-mini',
            distribution_rule: 'none',
            targets: [
              expect.objectContaining({
                source_instance_id: 'provider-openai-prod',
                source_instance_display_name: 'OpenAI Production',
                routing_enabled: true
              })
            ]
          })
        ]
      })
    );
    expect(modelProviderOptionsContract.providers[0]).not.toHaveProperty(
      'effective_instance_id'
    );
  });

  test('localizes model provider config schema labels and select options', async () => {
    vi.mocked(listConsoleModelProviderCatalog).mockResolvedValueOnce({
      ...modelProviderCatalogContract,
      locale_meta: {
        requested_locale: 'zh_Hans',
        resolved_locale: 'zh_Hans',
        user_preferred_locale: 'zh_Hans',
        accept_language: 'zh-Hans-CN,zh;q=0.9,en;q=0.8',
        fallback_locale: 'en_US',
        supported_locales: ['zh_Hans', 'en_US']
      },
      i18n_catalog: {
        'plugin.openai': {
          zh_Hans: {
            auth: {
              actions: {
                device_code: {
                  label: '设备授权'
                }
              }
            },
            fields: {
              transport_mode: {
                label: '传输模式',
                description: '选择 OpenAI Responses 的流式传输方式。',
                options: {
                  http_sse: {
                    label: 'HTTP SSE',
                    description: '默认使用 Responses HTTP 流式传输。'
                  },
                  responses_websocket: {
                    label: 'Responses WebSocket',
                    description: '显式使用 Responses WebSocket 传输。'
                  }
                }
              }
            }
          },
          en_US: {
            fields: {
              transport_mode: {
                label: 'Transport mode'
              }
            }
          }
        }
      },
      entries: [
        {
          ...modelProviderCatalogEntries[0],
          namespace: 'plugin.openai',
          auth: {
            actions: [
              {
                code: 'device_code',
                label: 'auth.actions.device_code.label',
                user_action_kinds: ['device_code']
              }
            ]
          },
          form_schema: [
            {
              key: 'transport_mode',
              field_type: 'enum',
              label: 'fields.transport_mode.label',
              control: 'select',
              required: false,
              advanced: true,
              description: 'fields.transport_mode.description',
              default_value: 'http_sse',
              options: [
                {
                  label: 'fields.transport_mode.options.http_sse.label',
                  value: 'http_sse',
                  description:
                    'fields.transport_mode.options.http_sse.description'
                },
                {
                  label:
                    'fields.transport_mode.options.responses_websocket.label',
                  value: 'responses_websocket',
                  description:
                    'fields.transport_mode.options.responses_websocket.description'
                }
              ]
            }
          ]
        }
      ]
    });

    const [entry] = await fetchSettingsModelProviderCatalog();

    expect(entry.form_schema[0]).toEqual(
      expect.objectContaining({
        key: 'transport_mode',
        label: '传输模式',
        description: '选择 OpenAI Responses 的流式传输方式。'
      })
    );
    expect(entry.form_schema[0].options).toEqual([
      {
        label: 'HTTP SSE',
        value: 'http_sse',
        description: '默认使用 Responses HTTP 流式传输。'
      },
      {
        label: 'Responses WebSocket',
        value: 'responses_websocket',
        description: '显式使用 Responses WebSocket 传输。'
      }
    ]);
    expect(entry.auth).toEqual({
      actions: [
        {
          code: 'device_code',
          label: '设备授权',
          user_action_kinds: ['device_code']
        }
      ]
    });
  });

  test('localizes plugin catalog entries and adapts installation requests', async () => {
    vi.mocked(listConsolePluginFamilies).mockResolvedValueOnce({
      locale_meta: {
        requested_locale: null,
        resolved_locale: 'zh_Hans',
        user_preferred_locale: null,
        accept_language: null,
        fallback_locale: 'en_US',
        supported_locales: ['zh_Hans', 'en_US']
      },
      i18n_catalog: {
        'plugin.openai_compatible': {
          zh_Hans: {
            plugin: {
              label: 'OpenAI 兼容插件',
              description:
                '面向 OpenAI 兼容 Chat Completions API 的 provider 插件。'
            },
            provider: {
              label: 'OpenAI Compatible'
            }
          },
          en_US: {
            plugin: {
              label: 'OpenAI-Compatible API Provider',
              description:
                'Provider plugin for services exposing an OpenAI-compatible Chat Completions API.'
            },
            provider: {
              label: 'OpenAI Compatible'
            }
          }
        }
      },
      entries: [pluginFamilyEntry]
    });
    vi.mocked(listConsoleOfficialPluginCatalog).mockResolvedValueOnce({
      source_kind: 'official_registry',
      source_label: '官方源',
      registry_url: 'https://official.example.com/official-registry.json',
      source_freshness: 'fresh',
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
      entries: [
        {
          plugin_id: '1flowbase.openai_compatible',
          plugin_type: 'model_provider',
          provider_code: 'openai_compatible',
          display_name: 'OpenAI Compatible',
          description:
            '面向 OpenAI 兼容 Chat Completions API 的 provider 插件。',
          icon: 'https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/runtime-extensions/model-providers/openai_compatible/_assets/icon.svg',
          protocol: 'openai_compatible',
          latest_version: '0.3.7',
          minimum_host_version: '0.1.0',
          current_host_version: '0.2.0',
          compatibility_status: 'compatible',
          compatibility_warning_reason: null,
          selected_artifact: {
            os: 'linux',
            arch: 'amd64',
            libc: 'musl',
            rust_target: 'x86_64-unknown-linux-musl',
            download_url: 'https://example.com/openai.1flowbasepkg',
            checksum: 'sha256:abc123',
            signature_algorithm: 'ed25519',
            signing_key_id: 'official-key-2026-04'
          },
          help_url: 'https://platform.openai.com/docs/api-reference',
          model_discovery_mode: 'hybrid',
          install_status: 'not_installed'
        }
      ]
    });

    expect(settingsPluginFamiliesQueryKey).toEqual([
      'settings',
      'plugins',
      'families'
    ]);
    expect(settingsOfficialPluginsQueryKey).toEqual([
      'settings',
      'plugins',
      'official-catalog'
    ]);

    await expect(fetchSettingsPluginFamilies()).resolves.toEqual([
      expect.objectContaining({
        provider_code: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        description: '面向 OpenAI 兼容 Chat Completions API 的 provider 插件。',
        plugin_type: 'model_provider',
        current_version: '0.3.7',
        installation_status: 'assigned'
      })
    ]);
    await expect(fetchSettingsOfficialPluginCatalog()).resolves.toEqual(
      expect.objectContaining({
        source_kind: 'official_registry',
        entries: expect.arrayContaining([
          expect.objectContaining({
            plugin_id: '1flowbase.openai_compatible',
            display_name: 'OpenAI Compatible',
            description:
              '面向 OpenAI 兼容 Chat Completions API 的 provider 插件。',
            plugin_type: 'model_provider',
            icon: 'https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/runtime-extensions/model-providers/openai_compatible/_assets/icon.svg',
            latest_version: '0.3.7'
          })
        ])
      })
    );
    await installSettingsOfficialPlugin('openai_compatible@0.2.0', 'csrf-123');

    await upgradeSettingsPluginFamilyLatest('openai_compatible', 'csrf-123');
    await switchSettingsPluginFamilyVersion(
      'openai_compatible',
      'installation-1',
      'csrf-123'
    );

    expect(listConsolePluginFamilies).toHaveBeenCalledWith({
      plugin_type: 'model_provider'
    });
    expect(listConsoleOfficialPluginCatalog).toHaveBeenCalledWith({
      plugin_type: 'model_provider',
      limit: 20
    });
    expect(installConsoleOfficialPlugin).toHaveBeenCalledWith(
      { plugin_id: 'openai_compatible@0.2.0' },
      'csrf-123'
    );

    expect(upgradeConsolePluginFamilyLatest).toHaveBeenCalledWith(
      'openai_compatible',
      'csrf-123'
    );
    expect(switchConsolePluginFamilyVersion).toHaveBeenCalledWith(
      'openai_compatible',
      { installation_id: 'installation-1' },
      'csrf-123'
    );
  });

  test.each([
    {
      case: 'resolved plugin label before fallback provider label',
      catalog: {
        fr_FR: {
          provider: { label: '' },
          plugin: { label: 'Plugin français' }
        },
        de_DE: { provider: { label: 'Deutscher Anbieter' } }
      },
      expected: 'Plugin français'
    },
    {
      case: 'provider label before plugin label in the same locale',
      catalog: {
        fr_FR: {
          provider: { label: 'Fournisseur' },
          plugin: { label: 'Plugin' }
        }
      },
      expected: 'Fournisseur'
    },
    {
      case: 'fallback locale before default locales',
      catalog: {
        zh_Hans: { provider: { label: '中文供应商' } },
        de_DE: { provider: { label: 'Deutscher Anbieter' } }
      },
      expected: 'Deutscher Anbieter'
    },
    {
      case: 'Chinese default before English and remaining locales',
      catalog: {
        it_IT: { provider: { label: 'Fornitore' } },
        en_US: { provider: { label: 'Provider' } },
        zh_Hans: { provider: { label: '中文供应商' } }
      },
      expected: '中文供应商'
    },
    {
      case: 'English default before remaining locales',
      catalog: {
        it_IT: { provider: { label: 'Fornitore' } },
        en_US: { provider: { label: 'Provider' } }
      },
      expected: 'Provider'
    },
    {
      case: 'remaining locales in catalog insertion order',
      catalog: {
        pt_BR: { plugin: { label: 'Plugin português' } },
        it_IT: { provider: { label: 'Fornitore' } }
      },
      expected: 'Plugin português'
    },
    {
      case: 'missing namespace',
      catalog: undefined,
      expected: 'openai_compatible'
    },
    {
      case: 'missing and empty bundles',
      catalog: { fr_FR: null, de_DE: {}, zh_Hans: { provider: { label: '' } } },
      expected: 'openai_compatible'
    }
  ])('resolves plugin labels with $case', async ({ catalog, expected }) => {
    vi.mocked(listConsolePluginFamilies).mockResolvedValueOnce({
      locale_meta: { resolved_locale: 'fr_FR', fallback_locale: 'de_DE' },
      i18n_catalog: catalog ? { [pluginFamilyEntry.namespace]: catalog } : {},
      entries: [pluginFamilyEntry]
    });

    const [entry] = await fetchSettingsPluginFamilies('fr_FR');

    expect(entry).toMatchObject({ display_name: expected, description: null });
  });

  test.each([
    { descriptionKey: null, expected: null },
    { descriptionKey: '', expected: null },
    { descriptionKey: 'plugin.missing', expected: null },
    { descriptionKey: 'plugin.empty', expected: null },
    { descriptionKey: 'plugin.description', expected: 'Fallback description' }
  ])(
    'resolves plugin description $descriptionKey',
    async ({ descriptionKey, expected }) => {
      vi.mocked(listConsolePluginFamilies).mockResolvedValueOnce({
        locale_meta: { resolved_locale: 'fr_FR', fallback_locale: 'de_DE' },
        i18n_catalog: {
          [pluginFamilyEntry.namespace]: {
            fr_FR: { plugin: { description: '', empty: '' } },
            de_DE: { plugin: { description: 'Fallback description' } }
          }
        },
        entries: [{ ...pluginFamilyEntry, description_key: descriptionKey }]
      });

      const [entry] = await fetchSettingsPluginFamilies();

      expect(entry.description).toBe(expected);
    }
  );

  test.each([
    { value: undefined, expected: undefined },
    { value: null, expected: null },
    { value: '', expected: '' },
    { value: 'Original text', expected: 'Original text' },
    { value: 'fields.empty', expected: 'fields.empty' },
    { value: 'fields.label', expected: 'Fallback label' },
    { value: 'fields.label', namespace: 'missing', expected: 'fields.label' }
  ])(
    'preserves schema text fallback for $value in $namespace',
    async ({ value, namespace = 'plugin.openai', expected }) => {
      const optionalText =
        value === undefined
          ? {}
          : { label: value, description: value, placeholder: value };
      vi.mocked(listConsoleModelProviderCatalog).mockResolvedValueOnce({
        ...modelProviderCatalogContract,
        locale_meta: { resolved_locale: 'fr_FR', fallback_locale: 'de_DE' },
        i18n_catalog: {
          'plugin.openai': {
            fr_FR: { fields: { label: '', empty: '' } },
            de_DE: { fields: { label: 'Fallback label' } }
          }
        },
        entries: [
          {
            ...modelProviderCatalogEntries[0],
            namespace,
            form_schema: [
              {
                key: 'text',
                field_type: 'enum',
                required: false,
                advanced: false,
                ...optionalText,
                options: [
                  {
                    label: value ?? 'Literal option',
                    value: 'option',
                    ...(typeof value === 'string' ? { description: value } : {})
                  }
                ]
              }
            ]
          }
        ]
      });

      const [entry] = await fetchSettingsModelProviderCatalog();
      const field = entry.form_schema[0];

      for (const key of ['label', 'description', 'placeholder'] as const) {
        expect(field[key]).toBe(expected);
        expect(Object.hasOwn(field, key)).toBe(value !== undefined);
      }
      expect(field.options?.[0]).toEqual({
        label: expected ?? 'Literal option',
        value: 'option',
        ...(typeof value === 'string' ? { description: expected } : {})
      });
    }
  );

  test('keeps host cache query keys and overview callback', async () => {
    expect(settingsHostInfrastructureCacheOverviewQueryKey).toEqual([
      'settings',
      'host-infrastructure',
      'cache'
    ]);
    expect(
      settingsHostInfrastructureCacheEntriesQueryKey('application-logs')
    ).toEqual([
      'settings',
      'host-infrastructure',
      'cache',
      'domains',
      'application-logs',
      'entries'
    ]);

    await fetchSettingsHostInfrastructureCacheOverview();

    expect(getConsoleHostInfrastructureCacheOverview).toHaveBeenCalledTimes(1);
  });

  test('keeps host memory query keys and request adapters', async () => {
    expect(settingsHostInfrastructureMemoryOverviewQueryKey).toEqual([
      'settings',
      'host-infrastructure',
      'memory'
    ]);
    expect(settingsHostInfrastructureMemoryStatsOverviewQueryKey).toEqual([
      'settings',
      'host-infrastructure',
      'memory',
      'stats'
    ]);
    expect(
      settingsHostInfrastructureMemoryEntriesQueryKey('session-store', {
        inspection_path: ['workspace-1'],
        cursor: 'cursor-1',
        limit: 25
      })
    ).toEqual([
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      'session-store',
      'entries',
      ['workspace-1'],
      'cursor-1',
      25,
      null
    ]);
    expect(
      settingsHostInfrastructureMemoryTreeQueryKey('session-store', {
        inspection_path: []
      })
    ).toEqual([
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      'session-store',
      'tree',
      [],
      null,
      null,
      null
    ]);
    expect(
      settingsHostInfrastructureMemoryStatsQueryKey('session-store', {
        inspection_path: ['workspace-1']
      })
    ).toEqual([
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      'session-store',
      'stats',
      ['workspace-1']
    ]);
    expect(
      settingsHostInfrastructureMemorySearchQueryKey('session-store', {
        q: 'session',
        inspection_path: ['workspace-1']
      })
    ).toEqual([
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      'session-store',
      'search',
      'session',
      ['workspace-1'],
      null,
      null,
      null
    ]);

    await fetchSettingsHostInfrastructureMemoryOverview();
    await fetchSettingsHostInfrastructureMemoryStatsOverview();

    await fetchSettingsHostInfrastructureMemoryStats('session-store', {
      inspection_path: ['workspace-1']
    });

    await revealSettingsHostInfrastructureMemoryEntry(
      'session-store',
      'session:1',
      'csrf-123',
      'full'
    );

    expect(getConsoleHostInfrastructureMemoryOverview).toHaveBeenCalledTimes(1);
    expect(
      getConsoleHostInfrastructureMemoryStatsOverview
    ).toHaveBeenCalledTimes(1);

    expect(getConsoleHostInfrastructureMemoryStats).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: ['workspace-1'] }
    );

    expect(revealConsoleHostInfrastructureMemoryEntry).toHaveBeenCalledWith(
      'session-store',
      'session:1',
      'csrf-123',
      'full'
    );
  });

  test('keeps file management query keys and list callbacks', async () => {
    expect(settingsFileStoragesQueryKey).toEqual([
      'settings',
      'files',
      'storages'
    ]);
    expect(settingsFileTablesQueryKey).toEqual(['settings', 'files', 'tables']);

    await fetchSettingsFileStorages();

    await fetchSettingsFileTables();

    expect(fetchConsoleFileStorages).toHaveBeenCalledTimes(1);

    expect(fetchConsoleFileTables).toHaveBeenCalledTimes(1);
  });
});
