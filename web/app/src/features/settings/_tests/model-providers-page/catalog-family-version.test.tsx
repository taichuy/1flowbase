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

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
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
vi.mock('../../api/console-navigation', () => consoleNavigationApi);
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

describe('ModelProvidersPage - catalog and family version', () => {
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
          label_key: 'auto.model_providers',
          navigation_slot: 'settings',
          order: 1
        }
      ],
      permission_bindings: []
    });
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

  test('renders provider family rows and upgrades to the latest version from the catalog version column', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    await waitFor(
      () => {
        expect(pluginsApi.fetchSettingsPluginFamilies).toHaveBeenCalled();
      },
      { timeout: 10_000 }
    );
    expect(await screen.findByText('0.1.0')).toBeInTheDocument();
    expect(
      screen.queryByText('当前使用 0.1.0，最新版本 0.2.0')
    ).not.toBeInTheDocument();

    const catalogRow = await screen.findByRole(
      'row',
      {
        name: /OpenAI Compatible/
      },
      { timeout: 10_000 }
    );

    expect(
      within(catalogRow).getByRole('button', { name: /更\s*新/ })
    ).toBeInTheDocument();
    expect(
      within(catalogRow).queryByRole('button', { name: '版本管理' })
    ).not.toBeInTheDocument();
    expect(within(catalogRow).getByText('可用')).toBeInTheDocument();
    expect(within(catalogRow).queryByText('当前节点可用')).not.toBeInTheDocument();
    expect(within(catalogRow).queryByText(/^期望版本：/)).not.toBeInTheDocument();
    expect(
      within(catalogRow).queryByText(/^当前节点版本：/)
    ).not.toBeInTheDocument();
    fireEvent.click(
      within(catalogRow).getByRole('button', { name: /更\s*新/ })
    );

    await waitFor(() => {
      expect(pluginsApi.upgradeSettingsPluginFamilyLatest).toHaveBeenCalledWith(
        'openai_compatible',
        'csrf-123'
      );
    });
  }, 20000);

  test('keeps official catalog registry errors out of the installed provider status area', async () => {
    authenticateAsModelProviderManager();
    pluginsApi.fetchSettingsOfficialPluginCatalog.mockRejectedValue(
      new Error('official plugin registry returned an error status')
    );

    const view = renderApp('/settings/model-providers');

    const catalogRow = await screen.findByRole(
      'row',
      {
        name: /OpenAI Compatible/
      },
      { timeout: 10_000 }
    );

    expect(catalogRow).toBeInTheDocument();
    expect(
      within(catalogRow).getByRole('button', { name: '管理' })
    ).toBeInTheDocument();
    expect(
      view.container.querySelector('.settings-section-surface__status')
        ?.textContent ?? ''
    ).not.toContain('official plugin registry returned an error status');
    expect(
      view.container.querySelector('.model-provider-panel__official')
    ).toHaveTextContent('official plugin registry returned an error status');
  });

  test('does not keep the installed provider table loading while only the local model-provider catalog is pending', async () => {
    authenticateAsModelProviderManager();
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockReturnValue(
      new Promise(() => undefined)
    );
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([]);

    renderApp('/settings/model-providers');

    expect(
      await screen.findByText('暂无可用供应商', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    expect(screen.queryByText('正在加载供应商目录...')).not.toBeInTheDocument();
    expect(await screen.findByText('可用实例')).toBeInTheDocument();
  });

  test('disables creating provider instances when the current node artifact is unavailable', async () => {
    authenticateAsModelProviderManager();
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
          local_version: '0.0.9',
          local_checksum: null,
          installed_path: '/tmp/plugins/openai_compatible/0.0.9',
          artifact_status: 'outdated',
          runtime_status: 'inactive',
          checked_at: '2026-04-18T10:00:00Z',
          last_error: 'local_version_outdated'
        },
        latest_version: '0.1.0',
        has_update: false,
        installed_versions: [
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

    renderApp('/settings/model-providers');

    const catalogRow = await screen.findByRole(
      'row',
      {
        name: /OpenAI Compatible/
      },
      { timeout: 10_000 }
    );

    expect(
      within(catalogRow).getByRole('button', { name: '新增' })
    ).toBeDisabled();
    expect(within(catalogRow).getByText('不可用')).toBeInTheDocument();
    expect(within(catalogRow).queryByText('可用')).not.toBeInTheDocument();
    expect(within(catalogRow).queryByText('当前节点缺失')).not.toBeInTheDocument();
    expect(within(catalogRow).queryByText(/^期望版本：/)).not.toBeInTheDocument();
    expect(
      within(catalogRow).getByText('当前节点版本：0.0.9')
    ).toBeInTheDocument();
    expect(
      within(catalogRow).getByRole('button', { name: '修复' })
    ).toBeInTheDocument();
  });

  test('switches provider family version and shows a follow-up warning in the instances modal', async () => {
    authenticateAsModelProviderManager();
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([
      {
        provider_code: 'openai_compatible',
        display_name: 'OpenAI Compatible',
        protocol: 'openai_compatible',
        help_url: 'https://platform.openai.com/docs/api-reference',
        default_base_url: 'https://api.openai.com/v1',
        model_discovery_mode: 'hybrid',
        current_installation_id: 'installation-2',
        current_version: '0.2.0',
        current_local_artifact: {
          node_id: 'test-node',
          installation_id: 'installation-2',
          local_version: '0.2.0',
          local_checksum: null,
          installed_path: '/tmp/plugins/openai_compatible/0.2.0',
          artifact_status: 'ready',
          runtime_status: 'inactive',
          checked_at: '2026-04-19T10:00:00Z',
          last_error: null
        },
        latest_version: '0.2.0',
        has_update: false,
        installed_versions: [
          {
            installation_id: 'installation-2',
            plugin_version: '0.2.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-19T09:00:00Z',
            is_current: true
          },
          {
            installation_id: 'installation-1',
            plugin_version: '0.1.0',
            source_kind: 'official_registry',
            trust_level: 'verified_official',
            created_at: '2026-04-18T09:00:00Z',
            is_current: false
          }
        ]
      }
    ]);

    renderApp('/settings/model-providers');

    await waitFor(() => {
      expect(pluginsApi.fetchSettingsPluginFamilies).toHaveBeenCalled();
    });

    const catalogRow = await screen.findByRole('row', {
      name: /OpenAI Compatible/
    });
    const versionSelect = within(catalogRow).getByRole('combobox', {
      name: '切换 OpenAI Compatible 版本'
    });
    fireEvent.mouseDown(versionSelect);
    fireEvent.click(await screen.findByText('0.1.0'));

    await waitFor(() => {
      expect(pluginsApi.switchSettingsPluginFamilyVersion).toHaveBeenCalledWith(
        'openai_compatible',
        'installation-1',
        'csrf-123'
      );
    });

    fireEvent.click(within(catalogRow).getByRole('button', { name: '管理' }));
    expect(
      await screen.findByText(
        '该供应商刚完成版本切换，建议刷新模型并验证关键实例。'
      )
    ).toBeInTheDocument();
  }, 20000);

  test('deletes a provider family after confirmation', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    const catalogRow = await screen.findByRole('row', {
      name: /OpenAI Compatible/
    });
    fireEvent.click(within(catalogRow).getByRole('button', { name: '删除' }));

    expect((await screen.findAllByText('删除供应商')).length).toBeGreaterThan(
      0
    );
    expect(
      screen.getByText(
        '删除后会一并清理该供应商的全部实例、安装记录和本地插件文件。'
      )
    ).toBeInTheDocument();

    const confirmDialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(confirmDialog).getByRole('button', { name: /删\s*除/ })
    );

    await waitFor(() => {
      expect(pluginsApi.deleteSettingsPluginFamily).toHaveBeenCalledWith(
        'openai_compatible',
        'csrf-123'
      );
    });
  }, 20000);

  test('renders catalog and instance metadata for view-only users without manage actions', async () => {
    authenticateWithPermissions([
      'state_model.view.all'
    ]);

    renderApp('/settings/model-providers');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/model-providers');
    });
    await waitFor(() => {
      expect(
        modelProvidersApi.fetchSettingsModelProviderCatalog
      ).toHaveBeenCalled();
      expect(
        modelProvidersApi.fetchSettingsModelProviderInstances
      ).toHaveBeenCalled();
    });

    await screen.findByText('可用实例', {}, { timeout: 10000 });
    expect(
      screen.queryByText(
        '先安装供应商，再配置 API 密钥实例。只有 ready 状态的实例会进入 agentFlow 的模型选项。'
      )
    ).not.toBeInTheDocument();
    expect(
      (await screen.findAllByText('OpenAI Compatible', {}, { timeout: 10000 }))
        .length
    ).toBeGreaterThanOrEqual(1);
    expect(
      screen.queryByRole('heading', { name: '当前实例' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '管理' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('OpenAI Production')).not.toBeInTheDocument();
  }, 10000);

  test('shows create and row-level manage actions when state_model.manage.all is present', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    await screen.findByText('可用实例', {}, { timeout: 10000 });

    expect(
      await screen.findByRole('button', { name: '管理' }, { timeout: 10000 })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '当前实例' })
    ).not.toBeInTheDocument();

    const catalogRow = await screen.findByRole(
      'row',
      {
        name: /OpenAI Compatible/
      },
      { timeout: 10000 }
    );
    expect(
      within(catalogRow).getByRole('button', { name: '管理' })
    ).toBeInTheDocument();
    expect(
      within(catalogRow).getByRole('button', { name: '新增' })
    ).toBeInTheDocument();
    expect(
      within(catalogRow).queryByRole('button', { name: '版本管理' })
    ).not.toBeInTheDocument();
    expect(
      within(catalogRow).queryByRole('link', { name: '文档' })
    ).not.toBeInTheDocument();
  }, 20000);

  test('switches provider version from the catalog version column', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    await waitFor(
      () => {
        expect(pluginsApi.fetchSettingsPluginFamilies).toHaveBeenCalled();
      },
      { timeout: 10_000 }
    );

    const versionSelect = await screen.findByRole(
      'combobox',
      {
        name: '切换 OpenAI Compatible 版本'
      },
      { timeout: 10_000 }
    );

    fireEvent.mouseDown(versionSelect);
    fireEvent.click(await screen.findByText('0.2.0'));

    await waitFor(() => {
      expect(pluginsApi.switchSettingsPluginFamilyVersion).toHaveBeenCalledWith(
        'openai_compatible',
        'installation-2',
        'csrf-123'
      );
    });
  }, 20000);

  test('renders provider catalog headers in the expected order', async () => {
    authenticateAsModelProviderManager();

    renderApp('/settings/model-providers');

    const headers = await screen.findAllByRole('columnheader');
    const expectedHeaders = ['操作', '名称', '状态', '版本', '说明'];
    const expectedHeaderSet = new Set(expectedHeaders);
    const catalogHeaders: string[] = [];
    for (const header of headers) {
      const text = header.textContent?.trim() ?? '';
      if (expectedHeaderSet.has(text)) {
        catalogHeaders.push(text);
      }
    }

    expect(catalogHeaders.slice(0, 5)).toEqual(expectedHeaders);
  }, 10000);
});
