import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { Grid } from 'antd';
import { vi } from 'vitest';
import {
  modelProviderCatalogEntries,
  primaryContractProviderModels
} from '../../../../test/model-provider-contract-fixtures';

const membersApi = vi.hoisted(() => ({
  settingsMembersQueryKey: ['settings', 'members'],
  fetchSettingsMembers: vi.fn(),
  createSettingsMember: vi.fn(),
  disableSettingsMember: vi.fn(),
  enableSettingsMember: vi.fn(),
  deleteSettingsMember: vi.fn(),
  resetSettingsMemberPassword: vi.fn(),
  replaceSettingsMemberRoles: vi.fn()
}));

const rolesApi = vi.hoisted(() => ({
  settingsRolesQueryKey: ['settings', 'roles'],
  settingsRolePermissionsQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'permissions'
  ]),
  fetchSettingsRoles: vi.fn(),
  createSettingsRole: vi.fn(),
  updateSettingsRole: vi.fn(),
  deleteSettingsRole: vi.fn(),
  fetchSettingsRolePermissions: vi.fn(),
  replaceSettingsRolePermissions: vi.fn()
}));

const permissionsApi = vi.hoisted(() => ({
  settingsPermissionsQueryKey: ['settings', 'permissions'],
  fetchSettingsPermissions: vi.fn()
}));

const docsApi = vi.hoisted(() => ({
  settingsApiDocsCatalogQueryKey: ['settings', 'docs', 'catalog'],
  settingsApiDocsCategoryOperationsQueryKey: vi.fn((categoryId: string) => [
    'settings',
    'docs',
    'category',
    categoryId,
    'operations'
  ]),
  settingsApiDocsOperationSpecQueryKey: vi.fn((operationId: string) => [
    'settings',
    'docs',
    'operation',
    operationId,
    'openapi'
  ]),
  fetchSettingsApiDocsCatalog: vi.fn(),
  fetchSettingsApiDocsCategoryOperations: vi.fn(),
  fetchSettingsApiDocsOperationSpec: vi.fn()
}));

const modelProvidersApi = vi.hoisted(() => ({
  settingsModelProviderCatalogQueryKey: [
    'settings',
    'model-providers',
    'catalog'
  ],
  settingsModelProviderInstancesQueryKey: [
    'settings',
    'model-providers',
    'instances'
  ],
  settingsModelProviderOptionsQueryKey: [
    'settings',
    'model-providers',
    'options'
  ],
  settingsModelProviderModelsQueryKey: vi.fn((instanceId: string) => [
    'settings',
    'model-providers',
    'models',
    instanceId
  ]),
  fetchSettingsModelProviderCatalog: vi.fn(),
  fetchSettingsModelProviderInstances: vi.fn(),
  fetchSettingsModelProviderOptions: vi.fn(),
  fetchSettingsModelProviderMainInstance: vi.fn(),
  fetchSettingsModelProviderModels: vi.fn(),
  previewSettingsModelProviderModels: vi.fn(),
  createSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderMainInstance: vi.fn(),
  revealSettingsModelProviderSecret: vi.fn(),
  validateSettingsModelProviderInstance: vi.fn(),
  refreshSettingsModelProviderModels: vi.fn(),
  deleteSettingsModelProviderInstance: vi.fn()
}));

const pluginsApi = vi.hoisted(() => ({
  settingsOfficialPluginsQueryKey: ['settings', 'plugins', 'official-catalog'],
  settingsPluginFamiliesQueryKey: ['settings', 'plugins', 'families'],
  fetchSettingsPluginFamilies: vi.fn(),
  fetchSettingsOfficialPluginCatalog: vi.fn(),
  installSettingsOfficialPlugin: vi.fn(),
  uploadSettingsPluginPackage: vi.fn(),
  upgradeSettingsPluginFamilyLatest: vi.fn(),
  switchSettingsPluginFamilyVersion: vi.fn(),
  deleteSettingsPluginFamily: vi.fn(),
  installSettingsPluginCurrentNodeArtifact: vi.fn(),
  refreshSettingsPluginCurrentNodeArtifact: vi.fn(),
  fetchSettingsPluginTask: vi.fn()
}));

const systemRuntimeApi = vi.hoisted(() => ({
  settingsSystemRuntimeQueryKey: ['settings', 'system-runtime'],
  fetchSettingsSystemRuntimeProfile: vi.fn()
}));

const fileManagementApi = vi.hoisted(() => ({
  settingsFileStoragesQueryKey: ['settings', 'files', 'storages'],
  settingsFileTablesQueryKey: ['settings', 'files', 'tables'],
  fetchSettingsFileStorages: vi.fn(),
  createSettingsFileStorage: vi.fn(),
  fetchSettingsFileTables: vi.fn(),
  createSettingsFileTable: vi.fn(),
  updateSettingsFileTableBinding: vi.fn()
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../../api/members', () => membersApi);
vi.mock('../../api/roles', () => rolesApi);
vi.mock('../../api/permissions', () => permissionsApi);
vi.mock('../../api/api-docs', () => docsApi);
vi.mock('../../api/model-providers', () => modelProvidersApi);
vi.mock('../../api/plugins', () => pluginsApi);
vi.mock('../../api/system-runtime', () => systemRuntimeApi);
vi.mock('../../api/file-management', () => fileManagementApi);
vi.mock('../../api/console-navigation', () => consoleNavigationApi);
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="settings-page-scalar">Scalar</div>
}));

import { AppProviders } from '../../../../app/AppProviders';
import { AppRouterProvider } from '../../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  buildMainInstanceSettings,
  buildSettingsModelProviderInstances,
  buildSettingsModelProviderOptions
} from '../model-provider-test-fixtures';
import { SettingsModelProvidersSection } from '../../pages/settings-page/SettingsModelProvidersSection';
import { ModelProviderInstancesModal } from '../../components/model-providers/ModelProviderInstancesModal';

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function authenticateWithPermissions(
  permissions: string[],
  effectiveDisplayRole: 'member' | 'root' = 'member'
) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: effectiveDisplayRole,
      effective_display_role: effectiveDisplayRole,
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: effectiveDisplayRole,
      email: `${effectiveDisplayRole}@example.com`,
      phone: null,
      nickname: effectiveDisplayRole,
      name: effectiveDisplayRole,
      avatar_url: null,
      introduction: '',
      effective_display_role: effectiveDisplayRole,
      permissions
    }
  });
}

function authenticateAsModelProviderManager() {
  authenticateWithPermissions([
    'state_model.view.all',
    'state_model.manage.all'
  ]);
}

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

async function openProviderInstancesModal() {
  const catalogRow = await screen.findByRole(
    'row',
    {
      name: /OpenAI Compatible/
    },
    { timeout: 10_000 }
  );
  fireEvent.click(within(catalogRow).getByRole('button', { name: '管理' }));

  return screen.findByRole('dialog', { name: /OpenAI Compatible 实例/ });
}

async function openSourceManagementTab(modal: HTMLElement) {
  fireEvent.click(within(modal).getByRole('tab', { name: '来源管理' }));

  return within(modal).findByRole('switch', {
    name: '注入主实例 OpenAI Production'
  });
}

describe('ModelProvidersPage - instances modal', () => {
  beforeEach(() => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'settings.model-providers',
          surface_key: 'model-providers',
          path: '/settings/model-providers',
          surface_kind: 'system'
        }
      ],
      navigation_items: [
        {
          item_id: 'model-providers',
          route_id: 'settings.model-providers',
          parent_item_id: 'settings',
          label_key: 'auto.model_provider',
          navigation_slot: 'settings',
          order: 1
        }
      ],
      permission_bindings: []
    });
    resetAuthStore();
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: true,
      xl: false,
      xxl: false
    });
    membersApi.fetchSettingsMembers.mockResolvedValue([]);
    rolesApi.fetchSettingsRoles.mockResolvedValue([]);
    rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
      role_code: 'member',
      permission_codes: []
    });
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
    docsApi.fetchSettingsApiDocsCatalog.mockResolvedValue({
      title: '1flowbase API',
      version: '0.1.0',
      categories: []
    });
    docsApi.fetchSettingsApiDocsCategoryOperations.mockResolvedValue({
      id: 'console',
      label: '控制面',
      operations: []
    });
    docsApi.fetchSettingsApiDocsOperationSpec.mockResolvedValue({
      openapi: '3.1.0',
      info: { title: '1flowbase API', version: '0.1.0' },
      paths: {},
      components: {}
    });
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue(
      modelProviderCatalogEntries
    );
    modelProvidersApi.fetchSettingsModelProviderInstances.mockResolvedValue(
      buildSettingsModelProviderInstances()
    );
    modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue(
      buildSettingsModelProviderOptions()
    );
    modelProvidersApi.fetchSettingsModelProviderMainInstance.mockResolvedValue(
      buildMainInstanceSettings()
    );
    modelProvidersApi.updateSettingsModelProviderMainInstance.mockResolvedValue(
      buildMainInstanceSettings(false)
    );
    modelProvidersApi.previewSettingsModelProviderModels.mockResolvedValue({
      models: [
        {
          model_id: 'gpt-4o-mini',
          display_name: 'gpt-4o-mini',
          source: 'dynamic',
          supports_streaming: true,
          supports_tool_call: true,
          supports_multimodal: false,
          context_window: null,
          max_output_tokens: null,
          parameter_form: null,
          provider_metadata: {}
        }
      ],
      preview_token: 'preview-1',
      expires_at: '2026-04-22T12:00:00Z'
    });
    modelProvidersApi.fetchSettingsModelProviderModels.mockResolvedValue({
      provider_instance_id: 'provider-1',
      refresh_status: 'ready',
      source: 'hybrid',
      last_error_message: null,
      refreshed_at: '2026-04-18T10:01:00Z',
      models: primaryContractProviderModels
    });
    modelProvidersApi.revealSettingsModelProviderSecret.mockResolvedValue({
      key: 'api_key',
      value: 'super-secret'
    });
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([
      {
        provider_code: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        protocol: 'openai_compatible',
        help_url: 'https://platform.openai.com/docs/api-reference',
        default_base_url: 'https://api.openai.com/v1',
        model_discovery_mode: 'hybrid',
        current_installation_id: 'installation-1',
        current_version: '0.1.0',
        current_local_artifact: {
          node_id: 'test-node',
          installation_id: 'installation-1',
          local_version: '0.1.0',
          local_checksum: null,
          installed_path: '/tmp/plugins/openai_compatible/0.1.0',
          artifact_status: 'ready',
          runtime_status: 'inactive',
          checked_at: '2026-04-18T10:00:00Z',
          last_error: null
        },
        latest_version: '0.2.0',
        has_update: true,
        installed_versions: [
          {
            installation_id: 'installation-2',
            plugin_version: '0.2.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-19T09:00:00Z',
            is_current: false
          },
          {
            installation_id: 'installation-1',
            plugin_version: '0.1.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-18T09:00:00Z',
            is_current: true
          }
        ]
      }
    ]);
    pluginsApi.fetchSettingsOfficialPluginCatalog.mockResolvedValue({
      locale_meta: { resolved_locale: 'zh_Hans', fallback_locale: 'en_US' },
      page: { limit: 20, next_cursor: null },
      entries: []
    });
    pluginsApi.installSettingsOfficialPlugin.mockResolvedValue({
      installation: {
        id: 'installation-1',
        provider_code: 'openai_compatible',
        plugin_id: 'openai_compatible@0.1.0',
        plugin_version: '0.1.0',
        contract_version: '1flowbase.provider/v1',
        protocol: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        source_kind: 'official_registry',
        trust_level: 'verified_official',
        verification_status: 'valid',
        enabled: true,
        install_path: '/tmp/openai-compatible',
        checksum: 'sha256:abc123',
        signature_status: 'unsigned',
        signature_algorithm: null,
        signing_key_id: null,
        metadata_json: {},
        created_at: '2026-04-18T21:00:00Z',
        updated_at: '2026-04-18T21:00:00Z'
      },
      task: {
        id: 'task-1',
        installation_id: 'installation-1',
        workspace_id: 'workspace-1',
        provider_code: 'openai_compatible',
        task_kind: 'assign',
        status: 'success',
        status_message: 'assigned',
        detail_json: {},
        created_at: '2026-04-18T21:00:00Z',
        updated_at: '2026-04-18T21:00:00Z',
        finished_at: '2026-04-18T21:00:00Z'
      }
    });
    pluginsApi.upgradeSettingsPluginFamilyLatest.mockResolvedValue({
      id: 'task-upgrade',
      installation_id: 'installation-2',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'switch_version',
      status: 'success',
      status_message: 'switched',
      detail_json: {
        previous_installation_id: 'installation-1',
        previous_version: '0.1.0',
        target_installation_id: 'installation-2',
        target_version: '0.2.0',
        migrated_instance_count: 2
      },
      created_at: '2026-04-19T10:00:00Z',
      updated_at: '2026-04-19T10:00:00Z',
      finished_at: '2026-04-19T10:00:00Z'
    });
    pluginsApi.switchSettingsPluginFamilyVersion.mockResolvedValue({
      id: 'task-switch',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'switch_version',
      status: 'success',
      status_message: 'switched',
      detail_json: {
        previous_installation_id: 'installation-2',
        previous_version: '0.2.0',
        target_installation_id: 'installation-1',
        target_version: '0.1.0',
        migrated_instance_count: 2
      },
      created_at: '2026-04-19T10:05:00Z',
      updated_at: '2026-04-19T10:05:00Z',
      finished_at: '2026-04-19T10:05:00Z'
    });
    pluginsApi.deleteSettingsPluginFamily.mockResolvedValue({
      id: 'task-delete',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'uninstall',
      status: 'success',
      status_message: 'deleted',
      detail_json: {
        deleted_instance_count: 2,
        deleted_installation_count: 1
      },
      created_at: '2026-04-19T10:10:00Z',
      updated_at: '2026-04-19T10:10:00Z',
      finished_at: '2026-04-19T10:10:00Z'
    });
    pluginsApi.fetchSettingsPluginTask.mockResolvedValue({
      id: 'task-1',
      installation_id: 'installation-1',
      workspace_id: 'workspace-1',
      provider_code: 'openai_compatible',
      task_kind: 'assign',
      status: 'success',
      status_message: 'assigned',
      detail_json: {},
      created_at: '2026-04-18T21:00:00Z',
      updated_at: '2026-04-18T21:00:00Z',
      finished_at: '2026-04-18T21:00:00Z'
    });
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue({
      topology: { relationship: 'same_host' },
      hosts: []
    });
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([]);
    fileManagementApi.fetchSettingsFileTables.mockResolvedValue([]);
  });

  test(
    'wires preview and create submission from the model provider drawer into the settings api',
    { timeout: 20000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.createSettingsModelProviderInstance.mockResolvedValue({
        id: 'provider-3',
        installation_id: modelProviderCatalogEntries[0].installation_id,
        provider_code: modelProviderCatalogEntries[0].provider_code,
        protocol: modelProviderCatalogEntries[0].protocol,
        display_name: 'OpenAI Draft',
        status: 'ready',
        included_in_main: true,
        config_json: {
          base_url: 'https://api.openai.com/v1',
          api_key: 'supe****cret'
        },
        configured_models: [
          {
            model_id: 'gpt-4o-mini',
            enabled: true,
            context_window_override_tokens: null
          }
        ],
        enabled_model_ids: ['gpt-4o-mini'],
        catalog_refresh_status: 'ready',
        catalog_last_error_message: null,
        catalog_refreshed_at: '2026-04-18T10:05:00Z',
        model_count: 1
      });

      render(
        <AppProviders>
          <SettingsModelProvidersSection canManage />
        </AppProviders>
      );

      fireEvent.click(await screen.findByRole('button', { name: '新增' }));

      expect(await screen.findByText('API 密钥授权配置')).toBeInTheDocument();
      expect(screen.getByRole('switch', { name: '注入主实例' })).toBeChecked();

      fireEvent.change(screen.getByLabelText('API Endpoint'), {
        target: { value: 'https://api.openai.com/v1' }
      });
      fireEvent.change(screen.getByLabelText('API Key'), {
        target: { value: 'super-secret' }
      });
      fireEvent.change(screen.getByLabelText('名称'), {
        target: { value: 'OpenAI Draft' }
      });

      fireEvent.click(screen.getByRole('button', { name: /检\s*测/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.previewSettingsModelProviderModels
        ).toHaveBeenCalledWith(
          {
            installation_id: modelProviderCatalogEntries[0].installation_id,
            config: {
              base_url: 'https://api.openai.com/v1',
              api_key: 'super-secret'
            }
          },
          'csrf-123'
        );
      });

      const cachedModelSelect = screen.getByRole('combobox', {
        name: '缓存模型'
      });
      fireEvent.mouseDown(cachedModelSelect);
      fireEvent.click(await screen.findByText('gpt-4o-mini'));
      expect(screen.queryByLabelText('模型 ID 1')).not.toBeInTheDocument();

      fireEvent.click(
        within(screen.getByRole('dialog')).getByRole('button', { name: '新增' })
      );
      fireEvent.change(screen.getByLabelText('模型 ID 1'), {
        target: { value: 'gpt-4o-mini' }
      });
      expect(screen.getByRole('switch', { name: '启用模型 1' })).toBeChecked();

      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.createSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          {
            installation_id: modelProviderCatalogEntries[0].installation_id,
            display_name: 'OpenAI Draft',
            config: {
              base_url: 'https://api.openai.com/v1',
              api_key: 'super-secret'
            },
            configured_models: [
              {
                model_id: 'gpt-4o-mini',
                enabled: true,
                context_window_override_tokens: null,
                supports_multimodal: false
              }
            ],
            included_in_main: true,
            preview_token: 'preview-1'
          },
          'csrf-123'
        );
      });
    }
  );

  test(
    'opens provider instances modal from installed provider row as a management list',
    { timeout: 30000 },
    async () => {
      authenticateAsModelProviderManager();
      const options = buildSettingsModelProviderOptions();
      options.providers[0].model_groups[0] = {
        model_id: primaryContractProviderModels[0].model_id,
        distribution_rule: 'none',
        model: primaryContractProviderModels[0],
        targets: [
          {
            source_instance_id: 'provider-1',
            source_instance_display_name: 'OpenAI Production',
            model: primaryContractProviderModels[0]
          },
          {
            source_instance_id: 'provider-2',
            source_instance_display_name: 'OpenAI Backup',
            model: primaryContractProviderModels[0]
          }
        ]
      };
      modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValueOnce(
        options
      );

      render(
        <AppProviders>
          <SettingsModelProvidersSection canManage />
        </AppProviders>
      );

      const modal = await openProviderInstancesModal();
      expect(screen.getByTestId('fixed-height-modal-scroll-body')).toHaveClass(
        'model-provider-panel__instances-modal'
      );
      expect(
        within(modal).getByRole('tab', { name: '模型管理' })
      ).toHaveAttribute('aria-selected', 'true');
      expect(
        within(modal).getByRole('tab', { name: '来源管理' })
      ).toBeInTheDocument();
      expect(within(modal).getByText('聚合视图')).toBeInTheDocument();
      expect(
        within(modal).getByRole('switch', { name: '新实例自动注入主实例' })
      ).toBeInTheDocument();
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      expect(
        within(mainInstanceTable).getByRole('columnheader', {
          name: '模型 ID'
        })
      ).toBeInTheDocument();
      expect(
        within(mainInstanceTable).getByRole('columnheader', { name: '分组' })
      ).toBeInTheDocument();
      expect(
        within(mainInstanceTable).getByRole('row', {
          name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
        })
      ).toBeInTheDocument();
      const aggregatedRow = within(mainInstanceTable).getByRole('row', {
        name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
      });
      expect(within(aggregatedRow).getByText('gpt-4o-mini')).not.toHaveClass(
        'model-provider-panel__mono'
      );
      const productionTag = within(aggregatedRow).getByText('OpenAI Production');
      const backupTag = within(aggregatedRow).getByText('OpenAI Backup');
      expect(productionTag).toHaveClass('ant-tag-geekblue');
      expect(backupTag).toHaveClass('ant-tag-green');
      expect(
        within(modal).queryByRole('combobox', { name: '主实例' })
      ).not.toBeInTheDocument();
      expect(
        within(modal).queryByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).not.toBeInTheDocument();
      await openSourceManagementTab(modal);
      expect(
        within(modal).getByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).toBeInTheDocument();
      expect(
        within(modal).getByRole('switch', { name: '注入主实例 OpenAI Backup' })
      ).toBeInTheDocument();
      expect(
        within(modal).getAllByText(/gpt-4o-mini/i).length
      ).toBeGreaterThanOrEqual(1);
      expect(
        within(modal).getAllByText('OpenAI Backup').length
      ).toBeGreaterThanOrEqual(1);
    }
  );

  test(
    'AC-001 renders distribution_rule in the main-instance aggregation table and emits the DTO field value',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      let mainInstanceState = buildMainInstanceSettings(true, 'none');
      const distributionRuleChangeSpy = vi.fn(
        (
          modelId: string,
          distributionRule: 'none' | 'round_robin' | 'retry_round_robin'
        ) => {
          mainInstanceState = {
            ...mainInstanceState,
            model_distribution_rules:
              mainInstanceState.model_distribution_rules.map((rule) =>
                rule.model_id === modelId
                  ? { ...rule, distribution_rule: distributionRule }
                  : rule
              )
          };
        }
      );

      modelProvidersApi.fetchSettingsModelProviderMainInstance.mockImplementation(
        async () => mainInstanceState
      );
      render(
        <AppProviders>
          <ModelProviderInstancesModal
            open
            catalogEntry={modelProviderCatalogEntries[0]}
            providerDisplayName="OpenAI Compatible"
            mainInstance={mainInstanceState}
            modelGroups={
              buildSettingsModelProviderOptions().providers[0].model_groups
            }
            instances={buildSettingsModelProviderInstances()}
            updatingMainInstance={false}
            updatingInstanceId={null}
            refreshingCandidates={false}
            refreshing={false}
            deleting={false}
            canManage={true}
            versionSwitchNotice={null}
            onClose={vi.fn()}
            onEdit={vi.fn()}
            onRefreshCandidates={vi.fn()}
            onRefreshModels={vi.fn()}
            onDelete={vi.fn()}
            onToggleAutoIncludeNewInstances={vi.fn()}
            onChangeDistributionRule={distributionRuleChangeSpy}
            onToggleIncludedInMain={vi.fn()}
          />
        </AppProviders>
      );

      const modal = await screen.findByRole('dialog', {
        name: /OpenAI Compatible 实例/
      });
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      expect(
        within(mainInstanceTable).getByRole('columnheader', {
          name: '分发规则'
        })
      ).toBeInTheDocument();

      const distributionRuleSelect = mainInstanceTable.querySelector(
        'select.model-provider-panel__distribution-rule-select'
      );
      if (!(distributionRuleSelect instanceof HTMLSelectElement)) {
        throw new Error('expected distribution rule select');
      }
      expect(distributionRuleSelect).toBeEnabled();
      expect((await screen.findAllByText('无')).length).toBeGreaterThan(0);
      expect((await screen.findAllByText('轮询')).length).toBeGreaterThan(0);
      expect((await screen.findAllByText('重试轮询')).length).toBeGreaterThan(0);
      fireEvent.change(distributionRuleSelect, {
        target: { value: 'retry_round_robin' }
      });

      expect(distributionRuleChangeSpy).toHaveBeenCalledWith(
        primaryContractProviderModels[0].model_id,
        'retry_round_robin'
      );
    }
  );

  test(
    'runs candidate refresh and delete from the provider instances modal',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.refreshSettingsModelProviderModels.mockResolvedValue({
        provider_instance_id: 'provider-1',
        refresh_status: 'ready',
        source: 'remote',
        last_error_message: null,
        refreshed_at: '2026-04-18T10:03:00Z',
        models: []
      });
      modelProvidersApi.deleteSettingsModelProviderInstance.mockResolvedValue({
        deleted: true
      });

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      fireEvent.click(
        await screen.findByRole('button', {
          name: '刷新候选模型 OpenAI Production'
        })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.validateSettingsModelProviderInstance
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });

      fireEvent.click(
        screen.getByRole('button', { name: '刷新模型 OpenAI Production' })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.refreshSettingsModelProviderModels
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });

      fireEvent.click(
        screen.getByRole('button', { name: '删除实例 OpenAI Production' })
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.deleteSettingsModelProviderInstance
        ).toHaveBeenCalledWith('provider-1', 'csrf-123');
      });
    }
  );

  test(
    'explains source instance action buttons on hover',
    { timeout: 30_000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      const actions = [
        {
          buttonName: '编辑 API Key OpenAI Production',
          tooltip: '编辑来源实例的 API Key 和连接配置'
        },
        {
          buttonName: '刷新候选模型 OpenAI Production',
          tooltip: '验证当前配置，并预览可用候选模型'
        },
        {
          buttonName: '刷新模型 OpenAI Production',
          tooltip: '同步该来源实例的模型缓存'
        },
        {
          buttonName: '删除实例 OpenAI Production',
          tooltip: '删除该来源实例'
        }
      ];

      for (const action of actions) {
        fireEvent.mouseEnter(
          within(modal).getByRole('button', { name: action.buttonName })
        );
        expect(
          await screen.findByText(action.tooltip, undefined, { timeout: 10000 })
        ).toBeInTheDocument();
      }
    }
  );

  test(
    'opens the matching source instance drawer from a model group tag',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();
      const options = buildSettingsModelProviderOptions();
      options.providers[0].model_groups[0] = {
        model_id: primaryContractProviderModels[0].model_id,
        distribution_rule: 'none',
        model: primaryContractProviderModels[0],
        targets: [
          {
            source_instance_id: 'provider-1',
            source_instance_display_name: 'OpenAI Production',
            model: primaryContractProviderModels[0]
          },
          {
            source_instance_id: 'provider-2',
            source_instance_display_name: 'OpenAI Backup',
            model: primaryContractProviderModels[0]
          }
        ]
      };
      modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValueOnce(
        options
      );

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      const mainInstanceTable = within(modal).getByRole('table', {
        name: '主实例'
      });
      const aggregatedRow = within(mainInstanceTable).getByRole('row', {
        name: /gpt-4o-mini.*OpenAI Production.*OpenAI Backup/
      });

      fireEvent.click(
        within(aggregatedRow).getByRole('button', {
          name: 'OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByText('OpenAI Compatible 实例')).toBeInTheDocument();
    }
  );

  test(
    'keeps the provider instances modal open while opening the edit drawer',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByText('OpenAI Compatible 实例')).toBeInTheDocument();
      expect(
        within(modal).getByRole('tab', { name: '来源管理' })
      ).toHaveAttribute('aria-selected', 'true');
      expect(
        within(modal).getByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).toBeInTheDocument();
    }
  );

  test(
    'folds advanced provider fields into a collapsed section in the edit drawer',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByLabelText('API Endpoint')).toBeInTheDocument();
      expect(screen.queryByText('organization')).not.toBeInTheDocument();

      fireEvent.click(screen.getByText('高级配置（可选）'));

      expect(await screen.findByLabelText('organization')).toBeInTheDocument();
      expect(screen.getByLabelText('default_headers')).toBeInTheDocument();
    }
  );

  test(
    'hydrates cached model select from existing model catalog in edit mode',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);

      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      await waitFor(() => {
        expect(
          modelProvidersApi.fetchSettingsModelProviderModels
        ).toHaveBeenCalledWith('provider-1');
      });

      const cachedModelSelect = await screen.findByRole('combobox', {
        name: '缓存模型'
      });
      fireEvent.mouseDown(cachedModelSelect);
      expect(
        (await screen.findAllByText(primaryContractProviderModels[0].model_id))
          .length
      ).toBeGreaterThanOrEqual(1);
      expect(screen.getByLabelText('模型 ID 1')).toHaveValue('gpt-4o-mini');
    }
  );

  test(
    'masks api key by default and reveals it only after explicit action',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      expect(screen.getByDisplayValue('supe****cret')).toBeInTheDocument();
      expect(
        screen.queryByDisplayValue('super-secret')
      ).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: '显示 API Key' }));

      await waitFor(() => {
        expect(
          modelProvidersApi.revealSettingsModelProviderSecret
        ).toHaveBeenCalledWith('provider-1', 'api_key', 'csrf-123');
      });
      await waitFor(() => {
        expect(screen.getByLabelText('API Key')).toHaveValue('super-secret');
      });
    }
  );

  test(
    'AC-001 allows an unconfigured optional secret to be entered and submitted in edit mode',
    { timeout: 30000 },
    async () => {
      authenticateAsModelProviderManager();
      modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([
        {
          ...modelProviderCatalogEntries[0],
          form_schema: [
            ...modelProviderCatalogEntries[0].form_schema,
            {
              key: 'proxy_url',
              field_type: 'secret',
              label: '代理地址',
              required: false,
              advanced: true
            }
          ]
        }
      ]);

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      await openSourceManagementTab(modal);
      fireEvent.click(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      );

      expect(await screen.findByText('编辑 API 密钥配置')).toBeInTheDocument();
      fireEvent.click(screen.getByText('高级配置（可选）'));

      const proxyUrlInput = await screen.findByLabelText('代理地址');
      expect(proxyUrlInput).not.toHaveAttribute('readonly');
      expect(
        screen.queryByRole('button', { name: '显示 代理地址' })
      ).not.toBeInTheDocument();

      fireEvent.change(proxyUrlInput, {
        target: { value: 'http://127.0.0.1:7897' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(
          modelProvidersApi.updateSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          'provider-1',
          expect.objectContaining({
            config: expect.objectContaining({
              proxy_url: 'http://127.0.0.1:7897'
            })
          }),
          'csrf-123'
        );
      });
      expect(
        modelProvidersApi.revealSettingsModelProviderSecret
      ).not.toHaveBeenCalledWith('provider-1', 'proxy_url', 'csrf-123');
    }
  );
});
