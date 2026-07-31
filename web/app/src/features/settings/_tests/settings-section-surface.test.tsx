import { render, screen, waitFor, within } from '@testing-library/react';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  fetchSettingsModelProviderCatalog: vi.fn(),
  fetchSettingsModelProviderInstances: vi.fn(),
  fetchSettingsModelProviderOptions: vi.fn(),
  fetchSettingsModelProviderMainInstance: vi.fn(),
  createSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderInstance: vi.fn(),
  updateSettingsModelProviderMainInstance: vi.fn(),
  updateSettingsModelProviderInstanceInclusion: vi.fn(),
  previewSettingsModelProviderModels: vi.fn(),
  validateSettingsModelProviderInstance: vi.fn(),
  refreshSettingsModelProviderModels: vi.fn(),
  revealSettingsModelProviderSecret: vi.fn(),
  deleteSettingsModelProviderInstance: vi.fn(),
  fetchSettingsModelProviderModels: vi.fn()
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

const fileManagementApi = vi.hoisted(() => ({
  settingsFileStoragesQueryKey: ['settings', 'files', 'storages'],
  settingsFileTablesQueryKey: ['settings', 'files', 'tables'],
  fetchSettingsFileStorages: vi.fn(),
  createSettingsFileStorage: vi.fn(),
  deleteSettingsFileStorage: vi.fn(),
  fetchSettingsFileTables: vi.fn(),
  createSettingsFileTable: vi.fn(),
  deleteSettingsFileTable: vi.fn(),
  updateSettingsFileTableBinding: vi.fn()
}));

const authApi = vi.hoisted(() => ({
  fetchCurrentSession: vi.fn(),
  getAuthApiBaseUrl: vi.fn(() => 'http://127.0.0.1:7800'),
  getScalarApiBaseUrl: vi.fn(() => 'http://127.0.0.1:3100')
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../api/api-docs', () => docsApi);
vi.mock('../api/model-providers', () => modelProvidersApi);
vi.mock('../api/plugins', () => pluginsApi);
vi.mock('../api/file-management', () => fileManagementApi);
vi.mock('../api/console-navigation', () => consoleNavigationApi);
vi.mock('../../auth/api/session', () => authApi);
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="settings-page-scalar">Scalar</div>
}));

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

const settingsRouteRecords = {
  docs: {
    path: '/settings/docs',
    label_key: 'auto.api_documentation'
  },
  'model-providers': {
    path: '/settings/model-providers',
    label_key: 'auto.model_providers'
  },
  files: {
    path: '/settings/files',
    label_key: 'auto.file_management'
  }
} as const;

function settingsConsoleNavigation(
  sectionKeys: Array<keyof typeof settingsRouteRecords> = [
    'docs',
    'model-providers',
    'files'
  ]
) {
  return {
    route_definitions: sectionKeys.map((sectionKey) => ({
      route_id: `settings.${sectionKey}`,
      surface_key: sectionKey,
      path: settingsRouteRecords[sectionKey].path,
      surface_kind: 'system' as const
    })),
    navigation_items: sectionKeys.map((sectionKey, index) => ({
      item_id: `settings.${sectionKey}`,
      route_id: `settings.${sectionKey}`,
      parent_item_id: 'settings',
      label_key: settingsRouteRecords[sectionKey].label_key,
      navigation_slot: 'settings' as const,
      order: index + 1
    })),
    permission_bindings: []
  };
}

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function authenticateWithPermissions(permissions: string[]) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'root',
      name: 'root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
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

describe('settings section surface', () => {
  beforeEach(() => {
    resetAuthStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation()
    );
    useBreakpointSpy.mockReturnValue({
      xs: true,
      sm: true,
      md: true,
      lg: true,
      xl: false,
      xxl: false
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
    authApi.fetchCurrentSession.mockResolvedValue({
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      session: {
        id: 'session-123',
        user_id: 'user-1',
        tenant_id: 'tenant-1',
        current_workspace_id: 'workspace-1'
      },
      csrf_token: 'csrf-123',
      cookie_name: 'flowbase_console_session'
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
    fileManagementApi.fetchSettingsFileStorages.mockResolvedValue([]);
    fileManagementApi.fetchSettingsFileTables.mockResolvedValue([]);
  });

  test.each([
    {
      pathname: '/settings/docs',
      permissions: ['api_reference.view.all'],
      heading: null,
      level: null,
      visibleText: '暂无接口分类'
    },
    {
      pathname: '/settings/model-providers',
      permissions: ['state_model.view.all'],
      heading: '模型供应商',
      level: 5
    },
    {
      pathname: '/settings/files',
      permissions: ['file_table.view.own'],
      heading: null,
      level: null,
      visibleTab: '文件表'
    }
  ])(
    'renders %s inside a shared settings surface',
    async ({
      pathname,
      permissions,
      heading,
      level,
      visibleTab,
      visibleText
    }) => {
      authenticateWithPermissions(permissions);

      renderApp(pathname);

      await waitFor(() => {
        expect(window.location.pathname).toBe(pathname);
      });

      const surface = await screen.findByTestId(
        'settings-section-surface',
        {},
        { timeout: 10_000 }
      );

      expect(surface).toBeInTheDocument();
      if (heading && level) {
        await waitFor(() => {
          expect(
            within(surface).getByRole('heading', { name: heading, level })
          ).toBeInTheDocument();
        });
      }

      if (visibleTab) {
        await waitFor(() => {
          expect(
            within(surface).getByRole('tab', { name: visibleTab })
          ).toBeInTheDocument();
        });
      }

      if (visibleText) {
        await waitFor(() => {
          expect(
            within(surface).getAllByText(visibleText).length
          ).toBeGreaterThan(0);
        });
      }
    },
    10_000
  );
});
