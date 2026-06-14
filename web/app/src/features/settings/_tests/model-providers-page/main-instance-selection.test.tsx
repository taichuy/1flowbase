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

vi.mock('../../api/members', () => membersApi);
vi.mock('../../api/roles', () => rolesApi);
vi.mock('../../api/permissions', () => permissionsApi);
vi.mock('../../api/api-docs', () => docsApi);
vi.mock('../../api/model-providers', () => modelProvidersApi);
vi.mock('../../api/plugins', () => pluginsApi);
vi.mock('../../api/system-runtime', () => systemRuntimeApi);
vi.mock('../../api/file-management', () => fileManagementApi);
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
const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function authenticateWithPermissions(
  permissions: string[],
  effectiveDisplayRole: 'manager' | 'root' = 'manager'
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
    'route_page.view.all',
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
    { timeout: 20_000 }
  );
  fireEvent.click(within(catalogRow).getByRole('button', { name: '管理' }));

  return screen.findByRole('dialog', { name: /OpenAI Compatible 实例/ });
}

describe('ModelProvidersPage - main instance selection', () => {
  beforeEach(() => {
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
      role_code: 'manager',
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
    'updates provider defaults and child-instance inclusion from the provider instances modal',
    { timeout: 25_000 },
    async () => {
      authenticateAsModelProviderManager();

      let instancesState = buildSettingsModelProviderInstances();
      let mainInstanceState = buildMainInstanceSettings();

      modelProvidersApi.fetchSettingsModelProviderInstances.mockImplementation(
        async () => instancesState
      );
      modelProvidersApi.fetchSettingsModelProviderMainInstance.mockImplementation(
        async () => mainInstanceState
      );
      modelProvidersApi.updateSettingsModelProviderMainInstance.mockImplementation(
        async (providerCode, input, csrfToken) => {
          expect(providerCode).toBe('openai_compatible');
          expect(csrfToken).toBe('csrf-123');
          mainInstanceState = {
            provider_code: providerCode,
            auto_include_new_instances: input.auto_include_new_instances
          };

          return mainInstanceState;
        }
      );
      modelProvidersApi.updateSettingsModelProviderInstance.mockImplementation(
        async (instanceId, input, csrfToken) => {
          expect(csrfToken).toBe('csrf-123');
          const enabledModelIds: string[] = [];
          for (const model of input.configured_models) {
            if (model.enabled) {
              enabledModelIds.push(model.model_id);
            }
          }

          instancesState = instancesState.map((instance) =>
            instance.id === instanceId
              ? {
                  ...instance,
                  display_name: input.display_name,
                  included_in_main: input.included_in_main,
                  configured_models: input.configured_models,
                  enabled_model_ids: enabledModelIds,
                  config_json: input.config
                }
              : instance
          );

          return instancesState.find((instance) => instance.id === instanceId);
        }
      );

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      const autoIncludeSwitch = within(modal).getByRole('switch', {
        name: '新实例自动注入主实例'
      });
      expect(autoIncludeSwitch).toBeChecked();
      fireEvent.click(autoIncludeSwitch);

      await waitFor(() => {
        expect(
          modelProvidersApi.updateSettingsModelProviderMainInstance
        ).toHaveBeenCalledWith(
          'openai_compatible',
          {
            auto_include_new_instances: false
          },
          'csrf-123'
        );
      });

      const includedSwitch = within(modal).getByRole('switch', {
        name: '注入主实例 OpenAI Backup'
      });
      expect(includedSwitch).not.toBeChecked();
      fireEvent.click(includedSwitch);

      await waitFor(() => {
        expect(
          modelProvidersApi.updateSettingsModelProviderInstance
        ).toHaveBeenCalledWith(
          'provider-2',
          expect.objectContaining({
            display_name: 'OpenAI Backup',
            included_in_main: true
          }),
          'csrf-123'
        );
      });
    }
  );

  test(
    'renders grouped model previews and instance inclusion toggles in the main-instance modal',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      expect(await within(modal).findByText('聚合视图')).toBeInTheDocument();
      expect(
        within(modal).getAllByText(primaryContractProviderModels[0].model_id)
          .length
      ).toBeGreaterThanOrEqual(1);
      expect(
        within(modal).getByRole('switch', {
          name: '注入主实例 OpenAI Production'
        })
      ).toBeInTheDocument();
      expect(
        within(modal).getByRole('switch', { name: '注入主实例 OpenAI Backup' })
      ).toBeInTheDocument();
    }
  );

  test(
    'renders provider instances as a collapsible management list beneath the main-instance summary',
    { timeout: 15000 },
    async () => {
      authenticateAsModelProviderManager();

      renderApp('/settings/model-providers');

      const modal = await openProviderInstancesModal();
      expect(
        within(modal).getAllByText('OpenAI Production').length
      ).toBeGreaterThanOrEqual(1);
      expect(
        within(modal).queryByRole('combobox', { name: '主实例' })
      ).not.toBeInTheDocument();

      expect(
        within(modal).getAllByText('OpenAI Production').length
      ).toBeGreaterThanOrEqual(1);
      expect(within(modal).getByText('OpenAI Backup')).toBeInTheDocument();
      expect(
        within(modal).getByRole('button', {
          name: '编辑 API Key OpenAI Production'
        })
      ).toBeInTheDocument();
      expect(
        within(modal).queryByRole('button', {
          name: '编辑 API Key OpenAI Backup'
        })
      ).not.toBeInTheDocument();
      expect(
        within(modal).getAllByText(primaryContractProviderModels[0].model_id)
          .length
      ).toBeGreaterThanOrEqual(1);

      fireEvent.click(within(modal).getByText('OpenAI Backup'));

      expect(
        await within(modal).findByRole('button', {
          name: '编辑 API Key OpenAI Backup'
        })
      ).toBeInTheDocument();
      expect(
        within(modal).getAllByText(/gpt-4o-mini/).length
      ).toBeGreaterThanOrEqual(1);
      expect(within(modal).getByText('gpt-4.1-mini')).toBeInTheDocument();
    }
  );
});
