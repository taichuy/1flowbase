import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  updateSettingsFileStorage: vi.fn(),
  deleteSettingsFileStorage: vi.fn(),
  fetchSettingsFileTables: vi.fn(),
  createSettingsFileTable: vi.fn(),
  updateSettingsFileTableBinding: vi.fn(),
  deleteSettingsFileTable: vi.fn()
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../api/members', () => membersApi);
vi.mock('../api/roles', () => rolesApi);
vi.mock('../api/permissions', () => permissionsApi);
vi.mock('../api/api-docs', () => docsApi);
vi.mock('../api/model-providers', () => modelProvidersApi);
vi.mock('../api/plugins', () => pluginsApi);
vi.mock('../api/system-runtime', () => systemRuntimeApi);
vi.mock('../api/file-management', () => fileManagementApi);
vi.mock('../api/console-navigation', () => consoleNavigationApi);
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="settings-page-scalar">Scalar</div>
}));

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

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

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

function findFileTableTab() {
  return screen.findByRole('tab', { name: '文件表' }, { timeout: 10_000 });
}

describe('File management settings page', () => {
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
      role_code: 'member',
      permission_codes: []
    });
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'settings.files',
          surface_key: 'files',
          path: '/settings/files',
          surface_kind: 'system'
        }
      ],
      navigation_items: [
        {
          item_id: 'files',
          route_id: 'settings.files',
          parent_item_id: 'settings',
          label_key: 'auto.file_management',
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
    modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([]);
    modelProvidersApi.fetchSettingsModelProviderInstances.mockResolvedValue([]);
    modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue({
      locale_meta: {
        requested_locale: 'zh_Hans',
        resolved_locale: 'zh_Hans',
        fallback_locale: 'en_US',
        supported_locales: ['zh_Hans', 'en_US']
      },
      i18n_catalog: {},
      providers: []
    });
    modelProvidersApi.fetchSettingsModelProviderMainInstance.mockResolvedValue({
      provider_code: 'openai_compatible',
      auto_include_new_instances: true,
      revision: 0,
      model_routing_policies: []
    });
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([]);
    pluginsApi.fetchSettingsOfficialPluginCatalog.mockResolvedValue({
      locale_meta: { resolved_locale: 'zh_Hans', fallback_locale: 'en_US' },
      page: { limit: 20, next_cursor: null },
      entries: []
    });
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue({
      topology: { relationship: 'same_host' },
      hosts: []
    });
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([]);
    fileManagementApi.fetchSettingsFileTables.mockResolvedValue([]);
    fileManagementApi.createSettingsFileStorage.mockResolvedValue({});
    fileManagementApi.createSettingsFileTable.mockResolvedValue({});
  });

  test('root mode shows table management layout and creation entries', async () => {
    authenticateWithPermissions([], 'root');

    renderApp('/settings/files');

    expect(await findFileTableTab()).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '存储配置' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /新增/ })).toBeInTheDocument();
    expect(screen.getByPlaceholderText('搜索存储...')).toBeInTheDocument();
  }, 20_000);

  test('workspace mode only shows file table tab when table view is allowed', async () => {
    authenticateWithPermissions(['file_table.view.own']);

    renderApp('/settings/files');

    expect(await findFileTableTab()).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '存储配置' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /新增/ })
    ).not.toBeInTheDocument();
  });

  test('create-only workspace mode hides file table tab and keeps create entry visible', async () => {
    authenticateWithPermissions(['file_table.create.all']);
    fileManagementApi.fetchSettingsFileTables.mockClear();

    renderApp('/settings/files');

    expect(
      await screen.findByText(
        '暂无权限查看文件表列表，您可以创建一个新文件表。'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '文件表' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /新增/ })).toBeInTheDocument();
    expect(fileManagementApi.fetchSettingsFileTables).not.toHaveBeenCalled();
  });

  test('root mode opens the storage create drawer', async () => {
    authenticateWithPermissions([], 'root');

    renderApp('/settings/files');

    fireEvent.click(
      (await screen.findAllByRole('button', { name: /新增/ }))[0]
    );
    expect(await screen.findByText('新增存储配置')).toBeInTheDocument();

    fireEvent.click(document.body);
    await waitFor(() => {
      expect(screen.getByText('新增存储配置')).toBeInTheDocument();
    });
  });

  test('root mode creates a local storage with public base url in config_json', async () => {
    authenticateWithPermissions([], 'root');

    renderApp('/settings/files');

    fireEvent.click(
      (await screen.findAllByRole('button', { name: /新增/ }))[0]
    );

    fireEvent.change(await screen.findByLabelText('存储标识'), {
      target: { value: 'local-public' }
    });
    fireEvent.change(screen.getByLabelText('名称'), {
      target: { value: 'Local Public' }
    });
    fireEvent.change(screen.getByLabelText('公开访问 URL'), {
      target: { value: 'https://cdn.example.com/files' }
    });
    fireEvent.click(screen.getByRole('button', { name: /创\s*建/ }));

    await waitFor(() => {
      expect(fileManagementApi.createSettingsFileStorage).toHaveBeenCalledWith(
        expect.objectContaining({
          code: 'local-public',
          title: 'Local Public',
          driver_type: 'local',
          config_json: expect.objectContaining({
            public_base_url: 'https://cdn.example.com/files'
          })
        }),
        'csrf-123'
      );
    });
    expect(await screen.findByText('存储配置已创建')).toBeInTheDocument();
  });

  test('root mode shows existing public base url when editing local storage', async () => {
    authenticateWithPermissions([], 'root');
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([
      {
        id: 'storage-1',
        code: 'local-default',
        title: 'Local Default',
        driver_type: 'local',
        enabled: true,
        is_default: true,
        health_status: 'ready',
        last_health_error: null,
        config_json: {
          root_path: '/srv/files',
          public_base_url: 'https://cdn.example.com/files'
        },
        rule_json: {}
      }
    ]);

    renderApp('/settings/files');

    expect(await screen.findByText('Local Default')).toBeInTheDocument();
    fireEvent.click(screen.getByText('编辑'));

    expect(
      await screen.findByDisplayValue('https://cdn.example.com/files')
    ).toBeInTheDocument();
  });

  test('root mode opens the file table create drawer', async () => {
    authenticateWithPermissions([], 'root');

    renderApp('/settings/files');

    fireEvent.click(await screen.findByRole('tab', { name: '文件表' }));
    fireEvent.click(
      (await screen.findAllByRole('button', { name: /新增/ }))[0]
    );
    expect(await screen.findByText('新增文件表')).toBeInTheDocument();
  });
});
