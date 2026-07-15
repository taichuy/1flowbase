import { afterEach, describe, expect, expectTypeOf, test, vi } from 'vitest';
import {
  modelProviderCatalogContract,
  modelProviderCatalogEntries
} from '../../../../../test/model-provider-contract-fixtures';
import { modelProviderOptionsContract } from './settings-api-test-support';

import {
  fetchConsoleApiDocsCatalog,
  fetchConsoleApiDocsCategoryOperations,
  fetchConsoleApiOperationSpec,
  listConsoleMembers,
  createConsoleMember,
  updateConsoleMember,
  disableConsoleMember,
  enableConsoleMember,
  deleteConsoleMember,
  resetConsoleMemberPassword,
  changeConsolePassword,
  replaceConsoleMemberRoles,
  listConsolePermissions,
  listConsoleRoles,
  createConsoleRole,
  updateConsoleRole,
  deleteConsoleRole,
  fetchConsoleRolePermissions,
  replaceConsoleRolePermissions,
  listConsoleModelProviderCatalog,
  listConsoleModelProviderInstances,
  listConsoleSettingsModelProviderOptions,
  getConsoleModelProviderMainInstance,
  getConsoleModelProviderModels,
  previewConsoleModelProviderModels,
  createConsoleModelProviderInstance,
  updateConsoleModelProviderInstance,
  updateConsoleModelProviderMainInstance,
  validateConsoleModelProviderInstance,
  refreshConsoleModelProviderModels,
  revealConsoleModelProviderSecret,
  deleteConsoleModelProviderInstance,
  listConsolePluginFamilies,
  listConsoleOfficialPluginCatalog,
  installConsoleOfficialPlugin,
  fetchConsoleFileStorages,
  createConsoleFileStorage,
  fetchConsoleFileTables,
  createConsoleFileTable,
  updateConsoleFileTableBinding,
  uploadConsolePluginPackage,
  upgradeConsolePluginFamilyLatest,
  switchConsolePluginFamilyVersion,
  getConsolePluginTask,
  listConsoleHostInfrastructureProviders,
  getConsoleHostInfrastructureMemoryOverview,
  getConsoleHostInfrastructureMemoryStatsOverview,
  getConsoleHostInfrastructureMemoryStats,
  listConsoleHostInfrastructureMemoryEntries,
  listConsoleHostInfrastructureMemoryTree,
  searchConsoleHostInfrastructureMemoryEntries,
  revealConsoleHostInfrastructureMemoryEntry,
  getConsoleHostInfrastructureCacheOverview,
  listConsoleHostInfrastructureCacheEntries,
  revealConsoleHostInfrastructureCacheEntry,
  clearConsoleHostInfrastructureCacheEntry,
  clearConsoleHostInfrastructureCacheDomain,
  saveConsoleHostInfrastructureProviderConfig
} from '@1flowbase/api-client';
import type { ConsoleModelProviderInstance } from '@1flowbase/api-client';

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
  fetchSettingsApiDocsCatalog,
  fetchSettingsApiDocsCategoryOperations,
  fetchSettingsApiDocsOperationSpec
} from '../../api-docs';
import {
  settingsMembersQueryKey,
  fetchSettingsMembers,
  createSettingsMember,
  updateSettingsMember,
  disableSettingsMember,
  enableSettingsMember,
  deleteSettingsMember,
  resetSettingsMemberPassword,
  changeCurrentUserPassword,
  replaceSettingsMemberRoles
} from '../../members';
import {
  settingsPermissionsQueryKey,
  fetchSettingsPermissions
} from '../../permissions';
import {
  settingsRolesQueryKey,
  settingsRolePermissionsQueryKey,
  fetchSettingsRoles,
  createSettingsRole,
  updateSettingsRole,
  deleteSettingsRole,
  fetchSettingsRolePermissions,
  replaceSettingsRolePermissions
} from '../../roles';
import {
  settingsModelProviderCatalogQueryKey,
  settingsModelProviderInstancesQueryKey,
  settingsModelProviderOptionsQueryKey,
  settingsModelProviderModelsQueryKey,
  fetchSettingsModelProviderCatalog,
  fetchSettingsModelProviderInstances,
  fetchSettingsModelProviderOptions,
  fetchSettingsModelProviderMainInstance,
  fetchSettingsModelProviderModels,
  previewSettingsModelProviderModels,
  createSettingsModelProviderInstance,
  updateSettingsModelProviderInstance,
  updateSettingsModelProviderMainInstance,
  validateSettingsModelProviderInstance,
  refreshSettingsModelProviderModels,
  revealSettingsModelProviderSecret,
  deleteSettingsModelProviderInstance
} from '../../model-providers';
import type {
  CreateSettingsModelProviderInput,
  SettingsModelProviderInstance,
  SettingsModelProviderMainInstance,
  UpdateSettingsModelProviderInput,
  UpdateSettingsModelProviderMainInstanceInput
} from '../../model-providers';
import {
  settingsPluginFamiliesQueryKey,
  settingsOfficialPluginsQueryKey,
  fetchSettingsPluginFamilies,
  fetchSettingsOfficialPluginCatalog,
  installSettingsOfficialPlugin,
  uploadSettingsPluginPackage,
  upgradeSettingsPluginFamilyLatest,
  switchSettingsPluginFamilyVersion,
  fetchSettingsPluginTask
} from '../../plugins';
import {
  settingsFileStoragesQueryKey,
  settingsFileTablesQueryKey,
  fetchSettingsFileStorages,
  createSettingsFileStorage,
  fetchSettingsFileTables,
  createSettingsFileTable,
  updateSettingsFileTableBinding
} from '../../file-management';
import {
  clearSettingsHostInfrastructureCacheDomain,
  clearSettingsHostInfrastructureCacheEntry,
  fetchSettingsHostInfrastructureCacheEntries,
  fetchSettingsHostInfrastructureCacheOverview,
  fetchSettingsHostInfrastructureMemoryEntries,
  fetchSettingsHostInfrastructureMemoryOverview,
  fetchSettingsHostInfrastructureMemoryStatsOverview,
  fetchSettingsHostInfrastructureMemoryStats,
  fetchSettingsHostInfrastructureMemoryTree,
  fetchSettingsHostInfrastructureProviders,
  revealSettingsHostInfrastructureMemoryEntry,
  revealSettingsHostInfrastructureCacheEntry,
  saveSettingsHostInfrastructureProviderConfig,
  searchSettingsHostInfrastructureMemoryEntries,
  settingsHostInfrastructureCacheEntriesQueryKey,
  settingsHostInfrastructureCacheOverviewQueryKey,
  settingsHostInfrastructureMemoryEntriesQueryKey,
  settingsHostInfrastructureMemoryOverviewQueryKey,
  settingsHostInfrastructureMemoryStatsOverviewQueryKey,
  settingsHostInfrastructureMemoryStatsQueryKey,
  settingsHostInfrastructureMemorySearchQueryKey,
  settingsHostInfrastructureMemoryTreeQueryKey,
  settingsHostInfrastructureProvidersQueryKey
} from '../../host-infrastructure';

afterEach(() => {
  vi.clearAllMocks();
});

describe('settings api wrappers', () => {
  test('forwards api docs query keys and request helpers', async () => {
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
    await fetchSettingsApiDocsCategoryOperations('console', {
      offset: 20,
      limit: 20,
      q: 'members'
    });
    await fetchSettingsApiDocsOperationSpec('op-1');

    expect(fetchConsoleApiDocsCatalog).toHaveBeenCalledTimes(1);
    expect(fetchConsoleApiDocsCategoryOperations).toHaveBeenCalledWith(
      'console',
      {
        offset: 20,
        limit: 20,
        q: 'members'
      }
    );
    expect(fetchConsoleApiOperationSpec).toHaveBeenCalledWith('op-1');
  });

  test('forwards members, permissions, and roles helpers', async () => {
    const memberInput = { email: 'member@example.com', name: 'Member' };
    const memberUpdateInput = {
      name: 'Root Next',
      nickname: 'Captain Root',
      email: 'root-next@example.com',
      phone: '13900000000',
      introduction: 'updated root profile'
    };
    const passwordInput = { password: 'password-123' };
    const ownPasswordInput = {
      old_password: 'change-me',
      new_password: 'next-pass'
    };
    const memberRolesInput = { role_codes: ['member'] };
    const roleInput = { code: 'member', name: 'Member' };
    const roleUpdateInput = { name: 'Platform Member' };
    const rolePermissionsInput = { permission_codes: ['state_model.view.all'] };

    expect(settingsMembersQueryKey).toEqual(['settings', 'members']);
    expect(settingsPermissionsQueryKey).toEqual(['settings', 'permissions']);
    expect(settingsRolesQueryKey).toEqual(['settings', 'roles']);
    expect(settingsRolePermissionsQueryKey('member')).toEqual([
      'settings',
      'roles',
      'member',
      'permissions'
    ]);

    await fetchSettingsMembers();
    await createSettingsMember(memberInput as never, 'csrf-123');
    await updateSettingsMember('member-1', memberUpdateInput, 'csrf-123');
    await disableSettingsMember('member-1', 'csrf-123');
    await enableSettingsMember('member-1', 'csrf-123');
    await deleteSettingsMember('member-1', 'csrf-123');
    await resetSettingsMemberPassword(
      'member-1',
      passwordInput as never,
      'csrf-123'
    );
    await changeCurrentUserPassword(ownPasswordInput, 'csrf-123');
    await replaceSettingsMemberRoles(
      'member-1',
      memberRolesInput as never,
      'csrf-123'
    );
    await fetchSettingsPermissions();
    await fetchSettingsRoles();
    await createSettingsRole(roleInput as never, 'csrf-123');
    await updateSettingsRole('member', roleUpdateInput as never, 'csrf-123');
    await deleteSettingsRole('member', 'csrf-123');
    await fetchSettingsRolePermissions('member');
    await replaceSettingsRolePermissions(
      'member',
      rolePermissionsInput as never,
      'csrf-123'
    );

    expect(listConsoleMembers).toHaveBeenCalledTimes(1);
    expect(createConsoleMember).toHaveBeenCalledWith(memberInput, 'csrf-123');
    expect(updateConsoleMember).toHaveBeenCalledWith(
      'member-1',
      memberUpdateInput,
      'csrf-123'
    );
    expect(disableConsoleMember).toHaveBeenCalledWith('member-1', 'csrf-123');
    expect(enableConsoleMember).toHaveBeenCalledWith('member-1', 'csrf-123');
    expect(deleteConsoleMember).toHaveBeenCalledWith('member-1', 'csrf-123');
    expect(resetConsoleMemberPassword).toHaveBeenCalledWith(
      'member-1',
      passwordInput,
      'csrf-123'
    );
    expect(changeConsolePassword).toHaveBeenCalledWith(
      ownPasswordInput,
      'csrf-123'
    );
    expect(replaceConsoleMemberRoles).toHaveBeenCalledWith(
      'member-1',
      memberRolesInput,
      'csrf-123'
    );
    expect(listConsolePermissions).toHaveBeenCalledTimes(1);
    expect(listConsoleRoles).toHaveBeenCalledTimes(1);
    expect(createConsoleRole).toHaveBeenCalledWith(roleInput, 'csrf-123');
    expect(updateConsoleRole).toHaveBeenCalledWith(
      'member',
      roleUpdateInput,
      'csrf-123'
    );
    expect(deleteConsoleRole).toHaveBeenCalledWith('member', 'csrf-123');
    expect(fetchConsoleRolePermissions).toHaveBeenCalledWith('member');
    expect(replaceConsoleRolePermissions).toHaveBeenCalledWith(
      'member',
      rolePermissionsInput,
      'csrf-123'
    );
  });

  test('forwards model provider query keys and request helpers', async () => {
    const createInput = {
      installation_id: 'installation-1',
      display_name: 'OpenAI Production',
      included_in_main: true,
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
      preview_token: 'preview-1',
      config: {
        base_url: 'https://api.openai.com/v1'
      }
    } satisfies CreateSettingsModelProviderInput;
    const updateInput = {
      display_name: 'OpenAI Backup',
      included_in_main: false,
      configured_models: [
        {
          model_id: 'gpt-4o',
          enabled: true,
          context_window_override_tokens: null
        },
        {
          model_id: 'gpt-4o-mini',
          enabled: false,
          context_window_override_tokens: null
        }
      ],
      preview_token: 'preview-2',
      config: {
        base_url: 'https://backup.openai.example/v1'
      }
    } satisfies UpdateSettingsModelProviderInput;
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
    const mainInstanceInput = {
      auto_include_new_instances: false,
      model_distribution_rules: [
        {
          model_id: 'gpt-4o-mini',
          distribution_rule: 'none'
        }
      ]
    } satisfies UpdateSettingsModelProviderMainInstanceInput;

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
    vi.mocked(getConsoleModelProviderMainInstance).mockResolvedValueOnce({
      provider_code: 'openai_compatible',
      auto_include_new_instances: false,
      model_distribution_rules: [
        {
          model_id: 'gpt-4o-mini',
          distribution_rule: 'none'
        }
      ]
    });
    const fetchedMainInstance =
      await fetchSettingsModelProviderMainInstance('openai_compatible');
    await fetchSettingsModelProviderModels('provider-1');
    await previewSettingsModelProviderModels(
      {
        installation_id: 'installation-1',
        config: {
          base_url: 'https://api.openai.com/v1',
          api_key: 'super-secret'
        }
      } as never,
      'csrf-123'
    );
    await createSettingsModelProviderInstance(createInput as never, 'csrf-123');
    await updateSettingsModelProviderInstance(
      'provider-1',
      updateInput as never,
      'csrf-123'
    );
    const mainInstanceResult = await updateSettingsModelProviderMainInstance(
      'openai_compatible',
      mainInstanceInput,
      'csrf-123'
    );
    const validatedInstance = await validateSettingsModelProviderInstance(
      'provider-1',
      'csrf-123'
    );
    await refreshSettingsModelProviderModels('provider-1', 'csrf-123');
    await revealSettingsModelProviderSecret(
      'provider-1',
      'api_key',
      'csrf-123'
    );
    await deleteSettingsModelProviderInstance('provider-1', 'csrf-123');

    expect(listConsoleModelProviderCatalog).toHaveBeenCalledTimes(1);
    expect(listConsoleModelProviderInstances).toHaveBeenCalledTimes(1);
    expect(listConsoleSettingsModelProviderOptions).toHaveBeenCalledTimes(1);
    expect(getConsoleModelProviderMainInstance).toHaveBeenCalledWith(
      'openai_compatible'
    );
    expect(getConsoleModelProviderModels).toHaveBeenCalledWith('provider-1');
    expect(previewConsoleModelProviderModels).toHaveBeenCalledWith(
      {
        installation_id: 'installation-1',
        config: {
          base_url: 'https://api.openai.com/v1',
          api_key: 'super-secret'
        }
      },
      'csrf-123'
    );
    expect(createConsoleModelProviderInstance).toHaveBeenCalledWith(
      createInput,
      'csrf-123'
    );
    expect(updateConsoleModelProviderInstance).toHaveBeenCalledWith(
      'provider-1',
      updateInput,
      'csrf-123'
    );
    expect(updateConsoleModelProviderMainInstance).toHaveBeenCalledWith(
      'openai_compatible',
      mainInstanceInput,
      'csrf-123'
    );
    expect(validateConsoleModelProviderInstance).toHaveBeenCalledWith(
      'provider-1',
      'csrf-123'
    );
    expect(refreshConsoleModelProviderModels).toHaveBeenCalledWith(
      'provider-1',
      'csrf-123'
    );
    expect(validatedInstance.instance).toEqual(
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
        included_in_main: true,
        enabled_model_ids: ['gpt-4o-mini']
      })
    );
    expect(fetchedMainInstance).toEqual({
      provider_code: 'openai_compatible',
      auto_include_new_instances: false,
      model_distribution_rules: [
        {
          model_id: 'gpt-4o-mini',
          distribution_rule: 'none'
        }
      ]
    } satisfies SettingsModelProviderMainInstance);
    expect(mainInstanceResult).toEqual({
      provider_code: 'openai_compatible',
      auto_include_new_instances: true,
      model_distribution_rules: [
        {
          model_id: 'gpt-4o-mini',
          distribution_rule: 'none'
        }
      ]
    } satisfies SettingsModelProviderMainInstance);
    expect(validatedInstance.instance).not.toHaveProperty(
      'validation_model_id'
    );
    expect(validatedInstance.instance).not.toHaveProperty('last_validated_at');
    expect(validatedInstance.instance).not.toHaveProperty(
      'last_validation_status'
    );
    expect(validatedInstance.instance).not.toHaveProperty(
      'last_validation_message'
    );
    expect(validatedInstance.instance.included_in_main).toBe(true);
    expect(revealConsoleModelProviderSecret).toHaveBeenCalledWith(
      'provider-1',
      'api_key',
      'csrf-123'
    );
    expect(deleteConsoleModelProviderInstance).toHaveBeenCalledWith(
      'provider-1',
      'csrf-123'
    );
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
                source_instance_display_name: 'OpenAI Production'
              })
            ]
          })
        ]
      })
    );
    expect(modelProviderOptionsContract.providers[0]).not.toHaveProperty(
      'effective_instance_id'
    );
    expect(validatedInstance.instance).not.toHaveProperty('is_primary');
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
  });

  test('forwards plugin query keys and request helpers', async () => {
    const uploadFile = new File(['zip'], 'provider.zip', {
      type: 'application/zip'
    });
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
      entries: [
        {
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
        }
      ]
    });
    vi.mocked(listConsoleOfficialPluginCatalog).mockResolvedValueOnce({
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
        current_version: '0.3.7'
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
    await uploadSettingsPluginPackage(uploadFile, 'csrf-123');
    await upgradeSettingsPluginFamilyLatest('openai_compatible', 'csrf-123');
    await switchSettingsPluginFamilyVersion(
      'openai_compatible',
      'installation-1',
      'csrf-123'
    );
    await fetchSettingsPluginTask('task-1');

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
    expect(uploadConsolePluginPackage).toHaveBeenCalledWith(
      uploadFile,
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
    expect(getConsolePluginTask).toHaveBeenCalledWith('task-1');
  });

  test('forwards host infrastructure provider helpers', async () => {
    expect(settingsHostInfrastructureProvidersQueryKey).toEqual([
      'settings',
      'host-infrastructure',
      'providers'
    ]);

    await fetchSettingsHostInfrastructureProviders();
    await saveSettingsHostInfrastructureProviderConfig(
      'installation-1',
      'redis',
      {
        enabled_contracts: ['storage-ephemeral'],
        config_json: { host: 'localhost', port: 6379 }
      },
      'csrf-123'
    );

    expect(listConsoleHostInfrastructureProviders).toHaveBeenCalledTimes(1);
    expect(saveConsoleHostInfrastructureProviderConfig).toHaveBeenCalledWith(
      'installation-1',
      'redis',
      {
        enabled_contracts: ['storage-ephemeral'],
        config_json: { host: 'localhost', port: 6379 }
      },
      'csrf-123'
    );
  });

  test('forwards host infrastructure cache helpers', async () => {
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
    await fetchSettingsHostInfrastructureCacheEntries('application-logs');
    await revealSettingsHostInfrastructureCacheEntry(
      'application-logs',
      'application-logs:run:1',
      'csrf-123'
    );
    await clearSettingsHostInfrastructureCacheEntry(
      'application-logs',
      'application-logs:run:1',
      'csrf-123'
    );
    await clearSettingsHostInfrastructureCacheDomain(
      'application-logs',
      'csrf-123'
    );

    expect(getConsoleHostInfrastructureCacheOverview).toHaveBeenCalledTimes(1);
    expect(listConsoleHostInfrastructureCacheEntries).toHaveBeenCalledWith(
      'application-logs'
    );
    expect(revealConsoleHostInfrastructureCacheEntry).toHaveBeenCalledWith(
      'application-logs',
      'application-logs:run:1',
      'csrf-123'
    );
    expect(clearConsoleHostInfrastructureCacheEntry).toHaveBeenCalledWith(
      'application-logs',
      'application-logs:run:1',
      'csrf-123'
    );
    expect(clearConsoleHostInfrastructureCacheDomain).toHaveBeenCalledWith(
      'application-logs',
      'csrf-123'
    );
  });

  test('forwards host infrastructure memory helpers', async () => {
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
    await fetchSettingsHostInfrastructureMemoryEntries('session-store', {
      inspection_path: ['workspace-1']
    });
    await fetchSettingsHostInfrastructureMemoryTree('session-store', {
      inspection_path: []
    });
    await fetchSettingsHostInfrastructureMemoryStats('session-store', {
      inspection_path: ['workspace-1']
    });
    await searchSettingsHostInfrastructureMemoryEntries('session-store', {
      q: 'session',
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
    expect(listConsoleHostInfrastructureMemoryEntries).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: ['workspace-1'] }
    );
    expect(listConsoleHostInfrastructureMemoryTree).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: [] }
    );
    expect(getConsoleHostInfrastructureMemoryStats).toHaveBeenCalledWith(
      'session-store',
      { inspection_path: ['workspace-1'] }
    );
    expect(searchConsoleHostInfrastructureMemoryEntries).toHaveBeenCalledWith(
      'session-store',
      { q: 'session', inspection_path: ['workspace-1'] }
    );
    expect(revealConsoleHostInfrastructureMemoryEntry).toHaveBeenCalledWith(
      'session-store',
      'session:1',
      'csrf-123',
      'full'
    );
  });

  test('forwards file management query keys and request helpers', async () => {
    const storageInput = {
      code: 'local-default',
      title: 'Local Default',
      driver_type: 'local',
      enabled: true,
      is_default: true,
      config_json: {
        root_path: '/srv/files'
      },
      rule_json: {}
    };
    const tableInput = {
      code: 'workspace_assets',
      title: 'Workspace Assets'
    };
    const bindingInput = {
      bound_storage_id: 'storage-2'
    };

    expect(settingsFileStoragesQueryKey).toEqual([
      'settings',
      'files',
      'storages'
    ]);
    expect(settingsFileTablesQueryKey).toEqual(['settings', 'files', 'tables']);

    await fetchSettingsFileStorages();
    await createSettingsFileStorage(storageInput as never, 'csrf-123');
    await fetchSettingsFileTables();
    await createSettingsFileTable(tableInput as never, 'csrf-123');
    await updateSettingsFileTableBinding('table-1', bindingInput, 'csrf-123');

    expect(fetchConsoleFileStorages).toHaveBeenCalledTimes(1);
    expect(createConsoleFileStorage).toHaveBeenCalledWith(
      storageInput,
      'csrf-123'
    );
    expect(fetchConsoleFileTables).toHaveBeenCalledTimes(1);
    expect(createConsoleFileTable).toHaveBeenCalledWith(tableInput, 'csrf-123');
    expect(updateConsoleFileTableBinding).toHaveBeenCalledWith(
      'table-1',
      bindingInput,
      'csrf-123'
    );
  });
});
