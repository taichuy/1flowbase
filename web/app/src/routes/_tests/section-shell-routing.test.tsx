import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  settingsRoleConsolePolicyQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'console-policy'
  ]),
  settingsRolePermissionsQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'permissions'
  ]),
  settingsRoleDataPolicyQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'data-policy'
  ]),
  settingsRoleFrontstageRoutesQueryKey: vi.fn((roleCode: string) => [
    'settings',
    'roles',
    roleCode,
    'frontstage-routes'
  ]),
  fetchSettingsRoles: vi.fn(),
  createSettingsRole: vi.fn(),
  updateSettingsRole: vi.fn(),
  deleteSettingsRole: vi.fn(),
  fetchSettingsRoleConsolePolicy: vi.fn(),
  replaceSettingsRoleConsolePolicy: vi.fn(),
  fetchSettingsRolePermissions: vi.fn(),
  replaceSettingsRolePermissions: vi.fn(),
  fetchSettingsRoleFrontstageRoutes: vi.fn(),
  replaceSettingsRoleFrontstageRoutes: vi.fn(),
  fetchSettingsRoleDataPolicy: vi.fn(),
  replaceSettingsRoleDataPolicy: vi.fn()
}));

const permissionsApi = vi.hoisted(() => ({
  settingsPermissionsQueryKey: ['settings', 'permissions'],
  fetchSettingsPermissions: vi.fn(),
  settingsConsolePolicyCatalogQueryKey: vi.fn((locale: string) => [
    'settings',
    'console-policy-catalog',
    locale
  ]),
  fetchSettingsConsolePolicyCatalog: vi.fn()
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
  settingsModelProviderRequestLogsQueryKey: vi.fn(() => ['request-logs']),
  fetchSettingsModelProviderRequestLogs: vi.fn(),
  previewSettingsModelProviderModels: vi.fn(),
  createSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderMainInstance: vi.fn(),
  revealSettingsModelProviderSecret: vi.fn(),
  validateSettingsModelProviderInstance: vi.fn(),
  refreshSettingsModelProviderModels: vi.fn(),
  deleteSettingsModelProviderInstance: vi.fn()
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

const dataModelsApi = vi.hoisted(() => ({
  settingsAllDataModelsQueryKey: ['settings', 'data-models', 'models', 'all'],
  settingsDataSourcesQueryKey: ['settings', 'data-models', 'sources'],
  settingsDataModelsQueryKey: vi.fn((sourceId: string) => [
    'settings',
    'data-models',
    'models',
    sourceId
  ]),
  settingsDataModelScopeGrantsQueryKey: vi.fn(),
  settingsDataModelAdvisorFindingsQueryKey: vi.fn(),
  settingsDataModelRecordPreviewQueryKey: vi.fn(),
  fetchSettingsDataSourceInstances: vi.fn(),
  fetchSettingsAllDataModels: vi.fn(),
  fetchSettingsDataModels: vi.fn(),
  fetchSettingsDataModelScopeGrants: vi.fn(),
  fetchSettingsDataModelAdvisorFindings: vi.fn(),
  fetchSettingsDataModelRecordPreview: vi.fn(),
  updateSettingsDataSourceDefaults: vi.fn(),
  updateSettingsDataModel: vi.fn(),
  updateSettingsDataModelScopeGrant: vi.fn()
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

const networkCenterApi = vi.hoisted(() => ({
  settingsNetworkEgressProvidersQueryKey: [
    'settings',
    'network-center',
    'providers'
  ],
  settingsNetworkEgressPoolsQueryKey: ['settings', 'network-center', 'pools'],
  settingsNetworkEgressRoutesQueryKey: ['settings', 'network-center', 'routes'],
  fetchSettingsNetworkEgressProviders: vi.fn(),
  fetchSettingsNetworkEgressProviderTypes: vi.fn(),
  fetchSettingsNetworkEgressPools: vi.fn(),
  fetchSettingsNetworkEgressRoutes: vi.fn()
}));

const extensionsApi = vi.hoisted(() => ({
  settingsInstalledExtensionsQueryKey: vi.fn(() => ['extensions', 'installed']),
  settingsExtensionCatalogQueryKey: vi.fn(
    (
      category: string,
      query: { q?: string; slot_code?: string; cursor?: string }
    ) => ['extensions', category, query.q, query.slot_code, query.cursor]
  ),
  fetchSettingsInstalledExtensions: vi.fn(),
  fetchSettingsExtensionCatalog: vi.fn(),
  fetchSettingsExtensionCatalogEntry: vi.fn(),
  checkSettingsExtensionUpdates: vi.fn(),
  installSettingsExtension: vi.fn(),
  getSettingsExtensionRiskChallenge: vi.fn(),
  previewSettingsInstalledMcpExtension: vi.fn(),
  applySettingsInstalledMcpExtension: vi.fn(),
  getSettingsInstalledMcpExtensionConflict: vi.fn(),
  getSettingsInstalledMcpExtensionIntegrityChallenge: vi.fn()
}));

vi.mock('../../features/settings/api/members', () => membersApi);
vi.mock('../../features/settings/api/roles', () => rolesApi);
vi.mock('../../features/settings/api/permissions', () => permissionsApi);
vi.mock('../../features/settings/api/api-docs', () => docsApi);
vi.mock('../../features/settings/api/model-providers', () => modelProvidersApi);
vi.mock('../../features/settings/api/file-management', () => fileManagementApi);
vi.mock('../../features/settings/api/data-models', () => dataModelsApi);
vi.mock(
  '../../features/settings/api/console-navigation',
  () => consoleNavigationApi
);
vi.mock('../../features/settings/api/network-center', () => networkCenterApi);
vi.mock('../../features/settings/api/extensions', () => extensionsApi);

import { AppProviders } from '../../app/AppProviders';
import { AppRouterProvider } from '../../app/router';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';

const SECTION_REDIRECT_WAIT_OPTIONS = { timeout: 8_000 };
const SECTION_REDIRECT_TEST_TIMEOUT = 30_000;

const settingsRouteRecords = {
  'api-key-authentication': {
    label_key: 'auto.api_key_authentication',
    path: '/settings/api-key-authentication'
  },
  members: {
    label_key: 'auto.user_management',
    path: '/settings/members'
  },
  roles: {
    label_key: 'auto.permission_management',
    path: '/settings/roles'
  },
  'data-models': {
    label_key: 'auto.data_source',
    path: '/settings/data-models'
  },
  files: {
    label_key: 'auto.file_management',
    path: '/settings/files'
  },
  'model-providers': {
    label_key: 'auto.model_providers',
    path: '/settings/model-providers'
  },
  'extension-center': {
    label_key: 'auto.extension_center',
    path: '/settings/extension-center'
  },
  'network-center': {
    label_key: 'auto.network_center',
    path: '/settings/network-center/proxy-types'
  }
} as const;

function settingsConsoleNavigation(
  sectionKeys: Array<keyof typeof settingsRouteRecords>
) {
  return {
    route_definitions: sectionKeys.map((surface_key) => ({
      route_id: surface_key,
      surface_key,
      path: settingsRouteRecords[surface_key].path,
      surface_kind: 'system' as const
    })),
    navigation_items: sectionKeys.map((surface_key, index) => ({
      item_id: surface_key,
      route_id: surface_key,
      parent_item_id: 'settings',
      label_key: settingsRouteRecords[surface_key].label_key,
      navigation_slot: 'settings' as const,
      order: index + 1
    })),
    permission_bindings: []
  };
}

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
      email: 'user@example.com',
      phone: null,
      nickname: 'User',
      name: 'User',
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

describe('section shell routing', () => {
  beforeEach(() => {
    resetAuthStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['api-key-authentication'])
    );
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 0,
      next_cursor: null,
      entries: []
    });
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'start',
        catalog_page_number: 1,
        catalog_page_checksum: 'sha256:fixture',
        catalog_page_locator: 'fixture',
        limit: 20,
        next_cursor: null,
        total_entries: 0,
        entries: []
      })
    );
    membersApi.fetchSettingsMembers.mockResolvedValue([]);
    rolesApi.fetchSettingsRoles.mockResolvedValue([
      {
        code: 'member',
        name: 'Member',
        introduction: 'Default role',
        scope_kind: 'workspace',
        is_builtin: true,
        is_editable: true,
        auto_grant_new_permissions: false,
        is_default_member_role: true,
        permission_codes: []
      }
    ]);
    rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
      role_code: 'member',
      permission_codes: []
    });
    rolesApi.fetchSettingsRoleFrontstageRoutes.mockResolvedValue({
      role_code: 'member',
      tree: [],
      checked_page_ids: [],
      checked_tab_ids: []
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: []
    });
    rolesApi.fetchSettingsRoleDataPolicy.mockResolvedValue({
      role_code: 'member',
      default_policy: {
        can_view: false,
        can_create: false,
        can_update: false,
        can_delete: false,
        default_view_scope: 'own',
        default_update_scope: 'own',
        default_delete_scope: 'own'
      },
      model_policies: []
    });
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
      schema_version: '2026-07-15',
      locale: 'zh_Hans',
      group_mode_options: [],
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          label: '应用管理',
          description: null,
          operations: []
        },
        {
          kind: 'other',
          group_id: 'other.general',
          label: '其他设置',
          description: null,
          operations: []
        }
      ],
      resources: []
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
    modelProvidersApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
      items: [],
      total_count: 0,
      page: 1,
      page_size: 20
    });
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
      distribution_rule: 'none'
    });
    networkCenterApi.fetchSettingsNetworkEgressProviders.mockReset();
    networkCenterApi.fetchSettingsNetworkEgressProviders.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressPools.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressRoutes.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataSourceInstances.mockResolvedValue([]);
    dataModelsApi.fetchSettingsAllDataModels.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModels.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelScopeGrants.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelAdvisorFindings.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelRecordPreview.mockResolvedValue({
      items: [],
      total: 0
    });
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([]);
    fileManagementApi.fetchSettingsFileTables.mockResolvedValue([
      {
        id: 'table-1',
        code: 'attachments',
        title: 'Attachments',
        scope_kind: 'workspace',
        scope_id: 'workspace-1',
        model_definition_id: 'model-1',
        bound_storage_id: 'storage-1',
        bound_storage_title: 'Local Default',
        is_builtin: true,
        is_default: true,
        status: 'active'
      }
    ]);
  });

  test(
    'section shell redirects /me to /me/profile',
    async () => {
      authenticateWithPermissions([]);

      renderApp('/me');

      await waitFor(() => {
        expect(window.location.pathname).toBe('/me/profile');
      }, SECTION_REDIRECT_WAIT_OPTIONS);
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'redirects /settings to first backend registry section',
    async () => {
      authenticateWithPermissions(['user.view.all']);

      renderApp('/settings');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/api-key-authentication'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(screen.getByTestId('section-page-layout')).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'redirects /settings/docs to API key when registry omits docs and includes roles',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['api-key-authentication', 'roles'])
      );
      authenticateWithPermissions(['settings_feature.access.system.roles']);

      renderApp('/settings/docs');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/api-key-authentication'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(screen.getByTestId('section-page-layout')).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'redirects /settings/docs to API key when registry omits docs and includes data models',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['api-key-authentication', 'data-models'])
      );
      authenticateWithPermissions([
        'settings_feature.access.system.data-models'
      ]);

      renderApp('/settings/docs');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/api-key-authentication'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(screen.getByTestId('section-page-layout')).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'redirects /settings/docs to API key when registry omits docs and includes file management',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['api-key-authentication', 'files'])
      );
      authenticateWithPermissions(['settings_feature.access.system.files']);

      renderApp('/settings/docs');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/api-key-authentication'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(screen.getByTestId('section-page-layout')).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'denies a direct settings section URL when backend navigation exposes no visible settings sections',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
        route_definitions: [],
        navigation_items: [],
        permission_bindings: []
      });
      authenticateWithPermissions([]);

      renderApp('/settings/docs');

      expect(await screen.findByText('无权限访问')).toBeInTheDocument();
      expect(docsApi.fetchSettingsApiDocsCatalog).not.toHaveBeenCalled();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-001 redirects the legacy model provider URL to providers',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['model-providers'])
      );
      authenticateWithPermissions(['state_model.manage.all']);
      renderApp('/settings/model-providers');
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/model-providers/providers'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-002 keeps request logs on its independent URL',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['model-providers'])
      );
      authenticateWithPermissions(['state_model.manage.all']);
      renderApp('/settings/model-providers/request-logs');
      expect(window.location.pathname).toBe(
        '/settings/model-providers/request-logs'
      );
      await waitFor(() => {
        expect(
          modelProvidersApi.fetchSettingsModelProviderRequestLogs
        ).toHaveBeenCalled();
      }, SECTION_REDIRECT_WAIT_OPTIONS);
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test.each([
    [
      '/settings/network-center/proxy-types',
      'network-center-providers-shell',
      '代理类型'
    ],
    ['/settings/network-center/proxy-pools', 'network-center-pools-shell', '代理池'],
    [
      '/settings/network-center/routing-rules',
      'network-center-routes-shell',
      '路由规则'
    ]
  ])(
    'AC-004 keeps %s protected and projects the network center shell',
    async (pathname, shellTestId, activeTab) => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['network-center'])
      );
      authenticateWithPermissions([
        'settings_feature.access.system.network-center'
      ]);

      renderApp(pathname);

      expect(await screen.findByTestId(shellTestId)).toBeInTheDocument();
      expect(screen.getByRole('tablist')).toBeInTheDocument();
      expect(
        screen.getByRole('tab', { name: '代理类型' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('tab', { name: activeTab, selected: true })
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('heading', { name: activeTab, level: 2 })
      ).not.toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '代理池' })).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '路由规则' })).toBeInTheDocument();
      await waitFor(() => {
        if (pathname.endsWith('/routing-rules')) {
          expect(
            networkCenterApi.fetchSettingsNetworkEgressRoutes
          ).toHaveBeenCalled();
          return;
        }
        if (pathname.endsWith('/proxy-types')) {
          expect(
            networkCenterApi.fetchSettingsNetworkEgressProviderTypes
          ).toHaveBeenCalled();
          return;
        }
        expect(
          networkCenterApi.fetchSettingsNetworkEgressProviders
        ).toHaveBeenCalled();
      }, SECTION_REDIRECT_WAIT_OPTIONS);
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-004 restores, navigates, and refreshes the URL-driven network center tabs',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['network-center'])
      );
      authenticateWithPermissions([
        'settings_feature.access.system.network-center'
      ]);

      const view = renderApp('/settings/network-center/proxy-pools');

      expect(await screen.findByRole('tablist')).toBeInTheDocument();
      expect(
        screen.getByRole('tab', { name: '代理类型' })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('tab', { name: '代理池', selected: true })
      ).toBeInTheDocument();
      expect(screen.getByRole('tab', { name: '路由规则' })).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: '路由规则' }));
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/network-center/routing-rules'
        );
      });
      expect(
        screen.getByRole('tab', { name: '路由规则', selected: true })
      ).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: '代理类型' }));
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/network-center/proxy-types'
        );
      });
      expect(
        screen.getByRole('tab', { name: '代理类型', selected: true })
      ).toBeInTheDocument();

      act(() => window.history.back());
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/network-center/routing-rules'
        );
      });
      expect(
        await screen.findByRole('tab', { name: '路由规则', selected: true })
      ).toBeInTheDocument();

      view.unmount();
      renderApp(window.location.pathname);
      expect(
        await screen.findByRole('tab', { name: '路由规则', selected: true })
      ).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-004 does not render network center when backend navigation omits its permission projection',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['api-key-authentication'])
      );
      authenticateWithPermissions(['user.view.all']);

      renderApp('/settings/network-center/proxy-types');

      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/api-key-authentication'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(
        networkCenterApi.fetchSettingsNetworkEgressProviders
      ).not.toHaveBeenCalled();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-001 redirects the legacy roles URL to the canonical console policy tab',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['roles'])
      );
      authenticateWithPermissions(['role_permission.manage.all']);

      renderApp('/settings/roles');

      await waitFor(() => {
        expect(window.location.pathname).toBe('/settings/roles/console-policy');
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(
        await screen.findByRole(
          'tab',
          { name: '后台设置', selected: true },
          SECTION_REDIRECT_WAIT_OPTIONS
        )
      ).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-002 restores and navigates every role permission tab on its independent URL',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['roles'])
      );
      authenticateWithPermissions(['role_permission.manage.all']);

      renderApp('/settings/roles/table-general-policy');

      expect(
        await screen.findByRole(
          'tab',
          {
            name: '表-通用配置',
            selected: true
          },
          SECTION_REDIRECT_WAIT_OPTIONS
        )
      ).toBeInTheDocument();
      expect(window.location.pathname).toBe(
        '/settings/roles/table-general-policy'
      );

      const tabRoutes = [
        ['动态路由', 'dynamic-routes'],
        ['表-单独配置', 'table-single-policy'],
        ['后台设置', 'console-policy'],
        ['其他', 'other-policy']
      ] as const;

      for (const [tabLabel, pathSegment] of tabRoutes) {
        fireEvent.click(screen.getByRole('tab', { name: tabLabel }));
        await waitFor(() => {
          expect(window.location.pathname).toBe(
            `/settings/roles/${pathSegment}`
          );
        });
        expect(
          screen.getByRole('tab', { name: tabLabel, selected: true })
        ).toBeInTheDocument();
      }
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-003 replaces an unavailable role permission tab with the first available URL',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['roles'])
      );
      permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
        schema_version: '2026-07-15',
        locale: 'zh_Hans',
        group_mode_options: [],
        groups: [],
        resources: []
      });
      authenticateWithPermissions(['role_permission.manage.all']);

      renderApp('/settings/roles/other-policy');

      await waitFor(() => {
        expect(window.location.pathname).toBe('/settings/roles/dynamic-routes');
      }, SECTION_REDIRECT_WAIT_OPTIONS);
      expect(
        screen.getByRole('tab', { name: '动态路由', selected: true })
      ).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'D5-AC-005 replaces the Extension Center base URL with its canonical installed route',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['extension-center', 'model-providers'])
      );
      authenticateWithPermissions([], 'root');
      renderApp('/settings/extension-center');
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/extension-center/installed'
        );
      }, SECTION_REDIRECT_WAIT_OPTIONS);
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'D5-AC-006 keeps Extension Center category/cursor in browser history',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['extension-center', 'model-providers'])
      );
      extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
        async (
          category: string,
          query: { q?: string; slot_code?: string; cursor?: string }
        ) => ({
          category,
          catalog_page: query.cursor ?? 'start',
          catalog_page_number: query.cursor ? 2 : 1,
          catalog_page_checksum: 'sha256:fixture',
          catalog_page_locator: 'fixture',
          limit: 20,
          next_cursor:
            category === 'runtime-extensions' && !query.cursor
              ? 'cursor-2'
              : null,
          total_entries: 0,
          entries: []
        })
      );
      authenticateWithPermissions([], 'root');
      renderApp('/settings/extension-center/installed');
      expect(
        await screen.findByRole(
          'tab',
          {
            name: '已安装扩展',
            selected: true
          },
          SECTION_REDIRECT_WAIT_OPTIONS
        )
      ).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: 'mcp' }));
      await waitFor(() => {
        expect(window.location.pathname).toBe('/settings/extension-center/mcp');
      });
      expect(
        await screen.findByRole('tab', { name: 'mcp', selected: true })
      ).toBeInTheDocument();

      fireEvent.click(screen.getByRole('tab', { name: 'runtime-extensions' }));
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/extension-center/runtime-extensions'
        );
        expect(window.location.search).toBe('');
        expect(
          extensionsApi.fetchSettingsExtensionCatalog
        ).toHaveBeenCalledWith('runtime-extensions', {
          q: undefined,
          slot_code: undefined,
          cursor: undefined
        });
      });
      fireEvent.click(await screen.findByRole('button', { name: '下一页' }));
      await waitFor(() => {
        expect(window.location.search).toBe('?cursor=cursor-2');
        expect(
          extensionsApi.fetchSettingsExtensionCatalog
        ).toHaveBeenCalledWith('runtime-extensions', {
          q: undefined,
          slot_code: undefined,
          cursor: 'cursor-2'
        });
      });

      act(() => window.history.back());
      await waitFor(() => expect(window.location.search).toBe(''));
      expect(
        await screen.findByRole('tab', {
          name: 'runtime-extensions',
          selected: true
        })
      ).toBeInTheDocument();

      act(() => window.history.back());
      await waitFor(() => {
        expect(window.location.pathname).toBe('/settings/extension-center/mcp');
        expect(window.location.search).toBe('');
      });
      expect(
        await screen.findByRole('tab', { name: 'mcp', selected: true })
      ).toBeInTheDocument();
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'runtime-extensions',
        { q: undefined, slot_code: undefined, cursor: undefined }
      );

      act(() => window.history.forward());
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/extension-center/runtime-extensions'
        );
        expect(window.location.search).toBe('');
      });
      act(() => window.history.forward());
      await waitFor(() => {
        expect(window.location.search).toBe('?cursor=cursor-2');
      });
      expect(
        await screen.findByRole('tab', {
          name: 'runtime-extensions',
          selected: true
        })
      ).toBeInTheDocument();
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );

  test(
    'AC-003/AC-007 keeps remote search in URL history, resets cursor, and retains q across pagination',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['extension-center'])
      );
      extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
        async (
          category: string,
          query: { q?: string; slot_code?: string; cursor?: string }
        ) => ({
          category,
          catalog_page: query.cursor ?? 'start',
          catalog_page_number: query.cursor ? 2 : 1,
          catalog_page_checksum: 'sha256:search-fixture',
          catalog_page_locator: 'search-fixture',
          limit: 20,
          next_cursor: query.cursor ? null : 'search-cursor-2',
          total_entries: 0,
          entries: []
        })
      );
      authenticateWithPermissions([], 'root');
      renderApp(
        '/settings/extension-center/runtime-extensions?q=old&cursor=stale'
      );

      const search = await screen.findByRole('searchbox');
      expect(search).toHaveValue('old');
      fireEvent.change(search, { target: { value: 'openai' } });
      fireEvent.keyDown(search, { key: 'Enter', code: 'Enter' });

      await waitFor(() => {
        expect(window.location.search).toBe('?q=openai');
        expect(
          extensionsApi.fetchSettingsExtensionCatalog
        ).toHaveBeenCalledWith('runtime-extensions', {
          q: 'openai',
          slot_code: undefined,
          cursor: undefined
        });
      });

      fireEvent.click(await screen.findByRole('button', { name: '下一页' }));
      await waitFor(() => {
        const params = new URLSearchParams(window.location.search);
        expect(params.get('q')).toBe('openai');
        expect(params.get('cursor')).toBe('search-cursor-2');
      });

      fireEvent.click(screen.getByRole('tab', { name: 'agent-flow' }));
      await waitFor(() => {
        expect(window.location.pathname).toBe(
          '/settings/extension-center/agent-flow'
        );
        expect(window.location.search).toBe('');
      });
    },
    SECTION_REDIRECT_TEST_TIMEOUT
  );
});
