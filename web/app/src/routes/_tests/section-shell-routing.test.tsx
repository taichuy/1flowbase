import { render, screen, waitFor } from '@testing-library/react';
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

import { AppProviders } from '../../app/AppProviders';
import { AppRouterProvider } from '../../app/router';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';

const SECTION_REDIRECT_WAIT_OPTIONS = { timeout: 8_000 };
const SECTION_REDIRECT_TEST_TIMEOUT = 10_000;

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
      distribution_rule: 'none'
    });
    dataModelsApi.fetchSettingsDataSourceInstances.mockResolvedValue([]);
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
      authenticateWithPermissions([
        'settings_route.visible.settings.roles'
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
    'redirects /settings/docs to API key when registry omits docs and includes data models',
    async () => {
      consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
        settingsConsoleNavigation(['api-key-authentication', 'data-models'])
      );
      authenticateWithPermissions([
        'settings_route.visible.settings.data-models'
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
      authenticateWithPermissions([
        'settings_route.visible.settings.files'
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
});
