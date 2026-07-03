import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const echartsMock = vi.hoisted(() => ({
  chart: {
    dispose: vi.fn(),
    resize: vi.fn(),
    setOption: vi.fn()
  },
  init: vi.fn()
}));

const membersApi = vi.hoisted(() => ({
  settingsMembersQueryKey: ['settings', 'members'],
  fetchSettingsMembers: vi.fn(),
  createSettingsMember: vi.fn(),
  updateSettingsMember: vi.fn(),
  disableSettingsMember: vi.fn(),
  enableSettingsMember: vi.fn(),
  deleteSettingsMember: vi.fn(),
  resetSettingsMemberPassword: vi.fn(),
  changeCurrentUserPassword: vi.fn(),
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

const personalAccessTokensApi = vi.hoisted(() => ({
  settingsPersonalAccessTokensQueryKey: ['settings', 'personal-access-tokens'],
  settingsPersonalAccessTokenRoleOptionsQueryKey: [
    'settings',
    'personal-access-tokens',
    'role-options'
  ],
  fetchSettingsPersonalAccessTokens: vi.fn(),
  fetchSettingsPersonalAccessTokenRoleOptions: vi.fn(),
  createSettingsPersonalAccessToken: vi.fn(),
  revokeSettingsPersonalAccessToken: vi.fn()
}));

const authCenterApi = vi.hoisted(() => ({
  settingsAuthCenterOverviewQueryKey: ['settings', 'auth-center', 'overview'],
  fetchSettingsAuthCenterOverview: vi.fn(),
  enableSettingsAuthCenterAuthenticator: vi.fn(),
  updateSettingsAuthCenterAuthenticatorConfig: vi.fn(),
  createSettingsAuthCenterAuthenticator: vi.fn(),
  copySettingsAuthCenterAuthenticator: vi.fn(),
  deleteSettingsAuthCenterAuthenticator: vi.fn(),
  reorderSettingsAuthCenterAuthenticators: vi.fn()
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
  fetchSettingsFileTables: vi.fn(),
  createSettingsFileTable: vi.fn(),
  updateSettingsFileTableBinding: vi.fn()
}));

const hostInfrastructureApi = vi.hoisted(() => ({
  settingsHostInfrastructureProvidersQueryKey: [
    'settings',
    'host-infrastructure',
    'providers'
  ],
  settingsHostInfrastructureMemoryOverviewQueryKey: [
    'settings',
    'host-infrastructure',
    'memory'
  ],
  settingsHostInfrastructureMemoryStatsOverviewQueryKey: [
    'settings',
    'host-infrastructure',
    'memory',
    'stats'
  ],
  settingsHostInfrastructureMemoryEntriesQueryKey: vi.fn(
    (contractCode: string | null) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'entries'
    ]
  ),
  settingsHostInfrastructureMemoryTreeQueryKey: vi.fn(
    (contractCode: string | null) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'tree'
    ]
  ),
  settingsHostInfrastructureMemoryStatsQueryKey: vi.fn(
    (contractCode: string | null) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'stats'
    ]
  ),
  settingsHostInfrastructureMemorySearchQueryKey: vi.fn(
    (contractCode: string | null) => [
      'settings',
      'host-infrastructure',
      'memory',
      'contracts',
      contractCode,
      'search'
    ]
  ),
  fetchSettingsHostInfrastructureProviders: vi.fn(),
  saveSettingsHostInfrastructureProviderConfig: vi.fn(),
  fetchSettingsHostInfrastructureMemoryOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStatsOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStats: vi.fn(),
  fetchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  fetchSettingsHostInfrastructureMemoryTree: vi.fn(),
  searchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  revealSettingsHostInfrastructureMemoryEntry: vi.fn()
}));

const dataModelsApi = vi.hoisted(() => ({
  settingsDataSourcesQueryKey: ['settings', 'data-models', 'sources'],
  settingsDataModelsQueryKey: vi.fn((sourceId: string) => [
    'settings',
    'data-models',
    'models',
    sourceId
  ]),
  settingsDataModelScopeGrantsQueryKey: vi.fn((modelId: string) => [
    'settings',
    'data-models',
    'scope-grants',
    modelId
  ]),
  settingsDataModelAdvisorFindingsQueryKey: vi.fn((modelId: string) => [
    'settings',
    'data-models',
    'advisor',
    modelId
  ]),
  settingsDataModelRecordPreviewQueryKey: vi.fn((modelCode: string) => [
    'settings',
    'data-models',
    'record-preview',
    modelCode
  ]),
  fetchSettingsDataSourceInstances: vi.fn(),
  updateSettingsDataSourceDefaults: vi.fn(),
  fetchSettingsDataModels: vi.fn(),
  createSettingsDataModel: vi.fn(),
  updateSettingsDataModel: vi.fn(),
  deleteSettingsDataModel: vi.fn(),
  fetchSettingsDataModelScopeGrants: vi.fn(),
  createSettingsDataModelField: vi.fn(),
  updateSettingsDataModelField: vi.fn(),
  deleteSettingsDataModelField: vi.fn(),
  createSettingsDataModelScopeGrant: vi.fn(),
  updateSettingsDataModelScopeGrant: vi.fn(),
  fetchSettingsDataModelAdvisorFindings: vi.fn(),
  fetchSettingsDataModelRecordPreview: vi.fn()
}));

vi.mock('../api/members', () => membersApi);
vi.mock('../api/roles', () => rolesApi);
vi.mock('../api/permissions', () => permissionsApi);
vi.mock('../api/api-docs', () => docsApi);
vi.mock('../api/personal-access-tokens', () => personalAccessTokensApi);
vi.mock('../api/auth-center', () => authCenterApi);
vi.mock('../api/model-providers', () => modelProvidersApi);
vi.mock('../api/plugins', () => pluginsApi);
vi.mock('../api/system-runtime', () => systemRuntimeApi);
vi.mock('../api/file-management', () => fileManagementApi);
vi.mock('../api/host-infrastructure', () => hostInfrastructureApi);
vi.mock('../api/data-models', () => dataModelsApi);
vi.mock('echarts/core', () => ({
  init: echartsMock.init,
  use: vi.fn()
}));
vi.mock('echarts/charts', () => ({
  BarChart: {},
  FunnelChart: {},
  GaugeChart: {},
  LineChart: {},
  PieChart: {},
  RadarChart: {}
}));
vi.mock('echarts/components', () => ({
  GridComponent: {},
  LegendComponent: {},
  TitleComponent: {},
  TooltipComponent: {}
}));
vi.mock('echarts/renderers', () => ({
  CanvasRenderer: {}
}));
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () => <div data-testid="settings-page-scalar">Scalar</div>
}));
vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();

  return {
    ...actual,
    listConsoleMembers: membersApi.fetchSettingsMembers,
    createConsoleMember: membersApi.createSettingsMember,
    updateConsoleMember: membersApi.updateSettingsMember,
    disableConsoleMember: membersApi.disableSettingsMember,
    resetConsoleMemberPassword: membersApi.resetSettingsMemberPassword,
    changeConsolePassword: membersApi.changeCurrentUserPassword,
    replaceConsoleMemberRoles: membersApi.replaceSettingsMemberRoles,
    listConsoleRoles: rolesApi.fetchSettingsRoles,
    fetchConsoleRolePermissions: rolesApi.fetchSettingsRolePermissions,
    createConsoleRole: rolesApi.createSettingsRole,
    updateConsoleRole: rolesApi.updateSettingsRole,
    deleteConsoleRole: rolesApi.deleteSettingsRole,
    replaceConsoleRolePermissions: rolesApi.replaceSettingsRolePermissions,
    fetchConsolePermissions: permissionsApi.fetchSettingsPermissions,
    fetchConsoleSystemRuntimeProfile:
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
  };
});

import { AppProviders } from '../../../app/AppProviders';
import { AppRouterProvider } from '../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

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

function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    <AppProviders>
      <AppRouterProvider />
    </AppProviders>
  );
}

describe('SettingsPage', () => {
  beforeEach(() => {
    echartsMock.init.mockReturnValue(echartsMock.chart);
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
      categories: [
        {
          id: 'console',
          label: '控制面',
          operation_count: 0
        }
      ]
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
    personalAccessTokensApi.fetchSettingsPersonalAccessTokens.mockResolvedValue(
      []
    );
    personalAccessTokensApi.fetchSettingsPersonalAccessTokenRoleOptions.mockResolvedValue(
      [{ code: 'root', name: 'Root', scope_kind: 'system' }]
    );
    personalAccessTokensApi.createSettingsPersonalAccessToken.mockResolvedValue(
      {
        id: 'key-1',
        name: 'CI diagnostics',
        token: 'pat_new_secret',
        token_prefix: 'pat_new',
        key_kind: 'user_api_key',
        role_code: 'root',
        creator_user_id: 'user-1',
        tenant_id: 'tenant-1',
        scope_kind: 'workspace',
        scope_id: 'workspace-1',
        enabled: true,
        revoked: false,
        expires_at: null,
        last_used_at: null,
        created_at: '2026-06-22T00:00:00Z',
        updated_at: '2026-06-22T00:00:00Z'
      }
    );
    personalAccessTokensApi.revokeSettingsPersonalAccessToken.mockResolvedValue(
      undefined
    );
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-password-local',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          enabled: true,
          is_builtin: true,
          sort_order: 0,
          config_schema: [],
          config_values: {
            title: 'Password',
            enabled: true,
            description: null,
            extension_config: {}
          }
        }
      ]
    });
    authCenterApi.enableSettingsAuthCenterAuthenticator.mockResolvedValue(
      undefined
    );
    authCenterApi.updateSettingsAuthCenterAuthenticatorConfig.mockResolvedValue(
      undefined
    );
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
      auto_include_new_instances: true
    });
    pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue([]);
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
      provider_install_root: '/home/taichu/git/1flowbase/api/plugins',
      host_extension_dropin_root:
        '/home/taichu/git/1flowbase/api/plugins/host-extension/dropins',
      locale_meta: {
        requested_locale: null,
        resolved_locale: 'zh_Hans',
        source: 'fallback',
        fallback_locale: 'en_US',
        supported_locales: ['zh_Hans', 'en_US']
      },
      topology: {
        relationship: 'same_host'
      },
      services: {
        api_server: {
          reachable: true,
          service: 'api-server',
          status: 'ok',
          version: '0.1.0',
          host_fingerprint: 'host-1'
        },
        plugin_runner: {
          reachable: true,
          service: 'plugin-runner',
          status: 'ok',
          version: '0.1.0',
          host_fingerprint: 'host-1'
        }
      },
      hosts: [
        {
          host_fingerprint: 'host-1',
          platform: {
            os: 'linux',
            arch: 'amd64',
            libc: 'musl',
            rust_target_triple: 'x86_64-unknown-linux-musl'
          },
          cpu: {
            logical_count: 8
          },
          memory: {
            total_bytes: 17179869184,
            total_gb: 16,
            available_bytes: 8589934592,
            available_gb: 8,
            process_bytes: 1073741824,
            process_gb: 1
          },
          services: ['api-server', 'plugin-runner']
        }
      ]
    });
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
          root_path: '/srv/files'
        },
        rule_json: {}
      }
    ]);
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
    hostInfrastructureApi.fetchSettingsHostInfrastructureProviders.mockResolvedValue(
      []
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue(
      {
        can_manage: true,
        contracts: []
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue(
      {
        inspection_path: [],
        contracts: [],
        entry_count: 0,
        sensitive_entry_count: 0,
        total_value_size_bytes: 0
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        supported: true,
        inspection_path: [],
        entry_count: 0,
        sensitive_entry_count: 0,
        total_value_size_bytes: 0
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        supported: true,
        inspection_path: [],
        nodes: [],
        next_cursor: null,
        limit: 50,
        byte_limit: 65536,
        emitted_bytes: 0,
        truncated_by_byte_limit: false
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        capabilities: {
          list_entries: true,
          reveal_value: true
        },
        supported: true,
        entries: []
      }
    );
    dataModelsApi.fetchSettingsDataSourceInstances.mockResolvedValue([
      {
        id: 'main_source',
        source_kind: 'main_source',
        installation_id: 'main_source',
        source_code: 'main_source',
        display_name: '主数据源',
        status: 'ready',
        default_data_model_status: 'published',
        config_json: {},
        secret_ref: null,
        secret_version: null,
        catalog_refresh_status: null,
        catalog_last_error_message: null,
        catalog_refreshed_at: null
      }
    ]);
    dataModelsApi.fetchSettingsDataModels.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelScopeGrants.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelAdvisorFindings.mockResolvedValue([]);
    dataModelsApi.fetchSettingsDataModelRecordPreview.mockResolvedValue(null);
  });

  test('shows API 文档 only for root or api_reference.view.all', async () => {
    const rootView = (() => {
      authenticateWithPermissions([], 'root');
      return renderApp('/settings');
    })();

    await waitFor(
      () => {
        expect(window.location.pathname).toBe('/settings/docs');
      },
      { timeout: 5000 }
    );
    await waitFor(() => {
      expect(docsApi.fetchSettingsApiDocsCatalog).toHaveBeenCalled();
    });
    rootView.unmount();

    resetAuthStore();
    docsApi.fetchSettingsApiDocsCatalog.mockClear();
    authenticateWithPermissions([
      'route_page.view.all',
      'api_reference.view.all'
    ]);
    const view = renderApp('/settings');

    await waitFor(
      () => {
        expect(window.location.pathname).toBe('/settings/docs');
      },
      { timeout: 5000 }
    );
    await waitFor(() => {
      expect(docsApi.fetchSettingsApiDocsCatalog).toHaveBeenCalled();
    });
    view.unmount();

    resetAuthStore();
    authenticateWithPermissions(['route_page.view.all', 'user.view.all']);
    renderApp('/settings');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(
      screen.queryByRole('heading', { name: 'API 文档', level: 3 })
    ).not.toBeInTheDocument();
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
  }, 10000);

  test('renders /settings/members when user.view.all is present', async () => {
    authenticateWithPermissions(['route_page.view.all', 'user.view.all']);

    renderApp('/settings/members');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/members');
    });
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--wide',
      'section-page-layout--viewport'
    );
    expect(
      await screen.findByText(
        '重置密码会将目标账号密码重置为默认临时密码，并要求用户登录后立即修改。'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '新建用户' })
    ).not.toBeInTheDocument();
  });

  test('renders auth center actions and opens configuration drawer', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'user.view.all',
      'user.manage.all'
    ]);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: false,
          is_builtin: false,
          sort_order: 10,
          config_schema: [
            {
              key: 'issuer_url',
              label: 'issuer_url',
              type: 'string'
            },
            {
              key: 'allow_signup',
              label: 'allow_signup',
              type: 'boolean'
            }
          ],
          config_values: {
            description: 'Primary OIDC',
            extension_config: {
              issuer_url: 'https://idp.example.com',
              allow_signup: true
            }
          }
        }
      ]
    });

    renderApp('/settings/auth-center');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/auth-center');
    });
    expect(
      screen.queryByRole('heading', { name: '认证中心' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('password-local')).not.toBeInTheDocument();
    expect(screen.queryByText('auth-oidc-main')).not.toBeInTheDocument();
    expect(await screen.findByText('OIDC')).toBeInTheDocument();
    expect(
      screen.getAllByRole('columnheader').map((header) => header.textContent)
    ).toEqual(['序号', '名称', '分类', '说明', '启用', '操作']);
    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('oidc')).toBeInTheDocument();
    expect(screen.getByText('Primary OIDC')).toBeInTheDocument();
    expect(screen.queryByText('10')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '排序值' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '操作' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('switch'));
    await waitFor(() => {
      expect(
        authCenterApi.enableSettingsAuthCenterAuthenticator
      ).toHaveBeenCalledWith('auth-oidc-main', 'csrf-123');
    });

    fireEvent.click(screen.getByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: 'OIDC 配置' });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).queryByText('类型')).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('名称')).toHaveValue('OIDC');
    expect(within(dialog).getByLabelText('说明')).toHaveValue('Primary OIDC');
    expect(
      within(dialog).getByRole('switch', { name: '启用' })
    ).not.toBeChecked();
  });

  test('initializes auth center config form from authenticator fields', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'user.view.all',
      'user.manage.all'
    ]);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-password-local',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          enabled: true,
          is_builtin: true,
          sort_order: 0,
          config_schema: [],
          config_values: {
            description: 'Local password authentication',
            extension_config: {}
          }
        }
      ]
    });

    renderApp('/settings/auth-center');

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: 'Password 配置' });

    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('名称')).toHaveValue('Password');
    expect(within(dialog).getByLabelText('说明')).toHaveValue(
      'Local password authentication'
    );
    expect(within(dialog).getByRole('switch', { name: '启用' })).toBeChecked();
    const resizeHandle = within(dialog).getByRole('separator', {
      name: '调整认证器配置抽屉宽度'
    });
    expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');
    fireEvent.mouseDown(resizeHandle, { clientX: 500 });
    expect(document.body).toHaveClass('schema-form-drawer--resizing');
    fireEvent.mouseMove(document, { clientX: 460 });
    expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');
    fireEvent.mouseUp(document);
    await waitFor(() => {
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '560');
    });
    expect(document.body).not.toHaveClass(
      'schema-form-drawer--resizing'
    );
    fireEvent.keyDown(resizeHandle, { key: 'ArrowLeft' });
    expect(resizeHandle).toHaveAttribute('aria-valuenow', '600');
    fireEvent.keyDown(resizeHandle, { key: 'Home' });
    expect(resizeHandle).toHaveAttribute('aria-valuenow', '480');
    fireEvent.keyDown(resizeHandle, { key: 'End' });
    expect(resizeHandle).toHaveAttribute('aria-valuenow', '960');
    const footer = within(dialog)
      .getByRole('button', { name: /保\s*存/ })
      .closest('.ant-drawer-footer');
    expect(footer).not.toBeNull();
    expect(footer?.querySelector('.ant-flex-justify-start')).not.toBeNull();
    const footerButtons = within(footer as HTMLElement).getAllByRole('button');
    expect(footerButtons.map((button) => button.textContent)).toEqual([
      '保 存',
      '取 消'
    ]);
  });

  test('submits auth center config, refreshes the list, and closes the drawer', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'user.view.all',
      'user.manage.all'
    ]);
    authCenterApi.fetchSettingsAuthCenterOverview
      .mockResolvedValueOnce({
        default_authenticator_id: 'auth-oidc-main',
        supported_auth_types: ['password-local'],
        authenticators: [
          {
            id: 'auth-oidc-main',
            auth_type: 'oidc',
            title: 'OIDC',
            enabled: false,
            is_builtin: false,
            sort_order: 10,
            config_schema: [],
            config_values: {
              title: 'OIDC',
              enabled: false,
              description: 'Old description',
              extension_config: {
                issuer_url: 'https://idp.example.com'
              }
            }
          }
        ]
      })
      .mockResolvedValueOnce({
        default_authenticator_id: 'auth-oidc-main',
        supported_auth_types: ['password-local'],
        authenticators: [
          {
            id: 'auth-oidc-main',
            auth_type: 'oidc',
            title: 'OIDC Login',
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            config_schema: [],
            config_values: {
              title: 'OIDC Login',
              enabled: true,
              description: 'Primary OIDC login',
              extension_config: {
                issuer_url: 'https://idp.example.com'
              }
            }
          }
        ]
      });
    authCenterApi.updateSettingsAuthCenterAuthenticatorConfig.mockResolvedValue(
      {
        id: 'auth-oidc-main',
        auth_type: 'oidc',
        title: 'OIDC Login',
        enabled: true,
        is_builtin: false,
        sort_order: 10,
        config_schema: [],
        config_values: {
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login',
          extension_config: {
            issuer_url: 'https://idp.example.com'
          }
        }
      }
    );

    renderApp('/settings/auth-center');

    const editButton = await screen.findByRole('button', { name: '编辑' });
    fireEvent.click(editButton);
    const dialog = await screen.findByRole('dialog', { name: 'OIDC 配置' });

    expect(within(dialog).queryByLabelText('标识')).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('名称'), {
      target: { value: 'OIDC Login' }
    });
    fireEvent.change(within(dialog).getByLabelText('说明'), {
      target: { value: 'Primary OIDC login' }
    });
    fireEvent.click(within(dialog).getByRole('switch', { name: '启用' }));
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    await waitFor(() => {
      expect(
        authCenterApi.updateSettingsAuthCenterAuthenticatorConfig
      ).toHaveBeenCalledWith(
        'auth-oidc-main',
        {
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login'
        },
        'csrf-123'
      );
    });
    expect(screen.getByText('OIDC Login')).toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: 'OIDC Login 配置' })
      ).not.toBeInTheDocument();
    });
  });

  test('shows auth center config errors in the drawer', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'user.view.all',
      'user.manage.all'
    ]);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: [],
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });
    authCenterApi.updateSettingsAuthCenterAuthenticatorConfig.mockRejectedValue(
      new Error('permission denied')
    );

    renderApp('/settings/auth-center');

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });
    fireEvent.change(within(dialog).getByLabelText('名称'), {
      target: { value: 'OIDC Login' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));

    expect(
      await within(dialog).findByText('认证器配置保存失败')
    ).toBeInTheDocument();
  });

  test('shows auth center manage permission error in the drawer', async () => {
    authenticateWithPermissions(['route_page.view.all', 'user.view.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: [],
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });

    renderApp('/settings/auth-center');

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });

    expect(
      within(dialog).getByText('需要认证器管理权限。')
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('名称')).toBeDisabled();
    expect(within(dialog).getByRole('switch', { name: '启用' })).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: /保\s*存/ })
    ).toBeDisabled();
  });

  test('shows auth center csrf error in the drawer', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'user.view.all',
      'user.manage.all'
    ]);
    useAuthStore.setState({ csrfToken: null });
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-oidc-main',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-oidc-main',
          auth_type: 'oidc',
          title: 'OIDC',
          enabled: true,
          is_builtin: false,
          sort_order: 10,
          config_schema: [],
          config_values: {
            title: 'OIDC',
            enabled: true,
            description: 'Old description'
          }
        }
      ]
    });

    renderApp('/settings/auth-center');

    fireEvent.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: /OIDC.*配置/ });

    expect(
      within(dialog).getByText('缺少安全校验令牌，请刷新页面后重试。')
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('名称')).toBeDisabled();
    expect(within(dialog).getByRole('switch', { name: '启用' })).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: /保\s*存/ })
    ).toBeDisabled();
  });

  test('opens auth center configuration drawer when extension config fields are absent', async () => {
    authenticateWithPermissions(['route_page.view.all', 'user.view.all']);
    authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
      default_authenticator_id: 'auth-password-local',
      supported_auth_types: ['password-local'],
      authenticators: [
        {
          id: 'auth-password-local',
          auth_type: 'password-local',
          title: 'Password',
          enabled: true,
          is_builtin: true,
          sort_order: 0,
          config_schema: [],
          config_values: {
            title: 'Password',
            enabled: true,
            description: null,
            extension_config: {}
          }
        }
      ]
    });

    renderApp('/settings/auth-center');

    const editButton = await screen.findByRole('button', { name: '编辑' });
    fireEvent.click(editButton);

    expect(
      await screen.findByRole('dialog', { name: 'Password 配置' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('名称')).toBeDisabled();
    expect(screen.getByLabelText('名称')).toHaveValue('Password');
    expect(screen.getByRole('switch', { name: '启用' })).toBeChecked();
  });

  test('renders API key for signed-in users without management permissions', async () => {
    authenticateWithPermissions([]);

    renderApp('/settings/api-key-authentication');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /添加/ })).toBeInTheDocument();
    expect(
      personalAccessTokensApi.fetchSettingsPersonalAccessTokens
    ).toHaveBeenCalled();
  });

  test('allows root safe member edits while keeping destructive actions locked', async () => {
    authenticateWithPermissions(
      ['route_page.view.all', 'user.view.all', 'user.manage.all'],
      'root'
    );
    rolesApi.fetchSettingsRoles.mockResolvedValue([
      {
        code: 'operator',
        name: 'Operator',
        introduction: 'operator role',
        scope_kind: 'workspace',
        is_builtin: false,
        is_editable: true,
        auto_grant_new_permissions: false,
        is_default_member_role: false,
        permission_codes: []
      }
    ]);
    membersApi.updateSettingsMember.mockResolvedValue({
      id: 'user-1',
      account: 'root',
      email: 'root-next@example.com',
      phone: '13900000000',
      name: 'Root Next',
      nickname: 'Captain Root',
      introduction: 'updated root profile',
      default_display_role: 'root',
      email_login_enabled: true,
      phone_login_enabled: false,
      status: 'active',
      role_codes: ['root']
    });
    membersApi.replaceSettingsMemberRoles.mockResolvedValue(undefined);
    membersApi.changeCurrentUserPassword.mockResolvedValue(undefined);
    membersApi.fetchSettingsMembers.mockResolvedValue([
      {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        name: 'Root',
        nickname: 'Root',
        introduction: '',
        default_display_role: 'root',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        role_codes: ['root']
      },
      {
        id: 'manager-1',
        account: 'manager-1',
        email: 'manager-1@example.com',
        phone: null,
        name: 'Manager 1',
        nickname: 'Manager 1',
        introduction: '',
        default_display_role: 'manager',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        role_codes: ['manager']
      }
    ]);

    renderApp('/settings/members');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/members');
    });
    await waitFor(() => {
      expect(membersApi.fetchSettingsMembers).toHaveBeenCalled();
    });

    await screen.findByText('root@example.com', {}, { timeout: 10_000 });
    await screen.findByText('manager-1@example.com', {}, { timeout: 10_000 });
    const rows = screen.getAllByRole('row');
    const rootRow = rows.find((row) =>
      within(row).queryByText('root@example.com')
    );
    const managerRow = rows.find((row) =>
      within(row).queryByText('manager-1@example.com')
    );

    if (!rootRow || !managerRow) {
      throw new Error('Expected root and manager member rows to be rendered.');
    }

    expect(
      within(rootRow).getByRole('button', { name: /编辑$/ })
    ).toBeEnabled();
    expect(
      within(rootRow).getByRole('button', { name: /停用$/ })
    ).toBeDisabled();
    expect(
      within(rootRow).getByRole('button', { name: /重置密码$/ })
    ).toBeEnabled();
    expect(
      within(managerRow).getByRole('button', { name: /停用$/ })
    ).toBeEnabled();
    expect(
      within(managerRow).getByRole('button', { name: /重置密码$/ })
    ).toBeEnabled();
    expect(
      screen.queryByRole('columnheader', { name: '角色' })
    ).not.toBeInTheDocument();

    fireEvent.click(within(rootRow).getByRole('button', { name: /编辑$/ }));
    const profileDialog = await screen.findByRole('dialog', {
      name: /编辑用户资料/
    });
    fireEvent.change(within(profileDialog).getByLabelText('姓名'), {
      target: { value: 'Root Next' }
    });
    fireEvent.change(within(profileDialog).getByLabelText('昵称'), {
      target: { value: 'Captain Root' }
    });
    fireEvent.change(within(profileDialog).getByLabelText('邮箱'), {
      target: { value: 'root-next@example.com' }
    });
    fireEvent.change(within(profileDialog).getByLabelText('手机号'), {
      target: { value: '13900000000' }
    });
    fireEvent.change(within(profileDialog).getByLabelText('个人介绍'), {
      target: { value: 'updated root profile' }
    });
    expect(
      within(profileDialog).getByRole('combobox', { name: '角色' })
    ).toBeInTheDocument();
    fireEvent.click(
      within(profileDialog).getByRole('button', { name: /保\s*存/ })
    );
    await waitFor(() => {
      expect(membersApi.updateSettingsMember).toHaveBeenCalledWith(
        'user-1',
        {
          name: 'Root Next',
          nickname: 'Captain Root',
          email: 'root-next@example.com',
          phone: '13900000000',
          introduction: 'updated root profile'
        },
        'csrf-123'
      );
    });
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: /编辑用户资料/ })
      ).not.toBeInTheDocument();
    });

    fireEvent.click(within(rootRow).getByRole('button', { name: /重置密码$/ }));
    const passwordDialog = await screen.findByRole('dialog', {
      name: /重置密码/
    });
    fireEvent.change(within(passwordDialog).getByLabelText('当前密码'), {
      target: { value: 'change-me' }
    });
    fireEvent.change(within(passwordDialog).getByLabelText('新密码'), {
      target: { value: 'next-pass' }
    });
    fireEvent.change(within(passwordDialog).getByLabelText('确认新密码'), {
      target: { value: 'next-pass' }
    });
    fireEvent.click(
      within(passwordDialog).getByRole('button', { name: '确认重置' })
    );
    await waitFor(() => {
      expect(membersApi.changeCurrentUserPassword).toHaveBeenCalledWith(
        {
          old_password: 'change-me',
          new_password: 'next-pass'
        },
        'csrf-123'
      );
    });
  }, 20_000);

  test('redirects /settings/docs to API key when docs is hidden', async () => {
    authenticateWithPermissions(['route_page.view.all', 'user.view.all']);

    renderApp('/settings/docs');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(screen.getByTestId('section-page-layout')).toHaveClass(
      'section-page-layout--viewport'
    );
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
  });

  test('shows 数据源 when state_model.view.all is present', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'state_model.view.all'
    ]);

    renderApp('/settings/data-models');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/data-models');
    });
    expect(await screen.findByRole('link', { name: '数据源' })).toHaveAttribute(
      'href',
      '/settings/data-models'
    );
    expect(dataModelsApi.fetchSettingsDataSourceInstances).toHaveBeenCalled();
    expect(
      await screen.findByText('主数据源', {}, { timeout: 10000 })
    ).toBeInTheDocument();
  });

  test('shows 系统运行 when system_runtime.view.all is present', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'system_runtime.view.all'
    ]);

    renderApp('/settings/system-runtime');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/system-runtime');
    });
    expect(await screen.findByText('部署概览')).toBeInTheDocument();
    expect(screen.getByText('同机部署')).toBeInTheDocument();
    expect(screen.getByText('zh_Hans')).toBeInTheDocument();
    expect(screen.getByText('API Server')).toBeInTheDocument();
    expect(screen.getByText('Plugin Runner')).toBeInTheDocument();
    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalled();
  });

  test('shows 基础设施 and 内存观察 when plugin_config.view.all is present', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'plugin_config.view.all'
    ]);

    renderApp('/settings/host-infrastructure');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/host-infrastructure');
    });
    expect(
      await screen.findByRole('link', { name: '内存观察' }, { timeout: 10000 })
    ).toHaveAttribute('href', '/settings/memory-observation');
    expect(
      await screen.findByText(
        '安装、配置和启用会保存为待应用变更，重启 api-server 一次后生效。'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '内存观察' })
    ).not.toBeInTheDocument();
  });

  test('renders memory observation as a settings section route', async () => {
    authenticateWithPermissions([
      'route_page.view.all',
      'plugin_config.view.all'
    ]);
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryOverview.mockResolvedValue(
      {
        can_manage: true,
        contracts: [
          {
            contract_code: 'session-store',
            label: 'Sessions',
            provider_code: 'local',
            capabilities: {
              list_entries: true,
              list_tree: true,
              search_entries: true,
              reveal_value: true
            },
            supported: true
          }
        ]
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStats.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        capabilities: {
          list_entries: true,
          list_tree: true,
          search_entries: true,
          reveal_value: true
        },
        supported: true,
        inspection_path: [],
        entry_count: 1,
        sensitive_entry_count: 1,
        total_value_size_bytes: 317
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryStatsOverview.mockResolvedValue(
      {
        inspection_path: [],
        entry_count: 1,
        sensitive_entry_count: 1,
        total_value_size_bytes: 317,
        contracts: [
          {
            contract_code: 'session-store',
            label: 'Sessions',
            provider_code: 'local',
            capabilities: {
              list_entries: true,
              list_tree: true,
              search_entries: true,
              reveal_value: true
            },
            supported: true,
            inspection_path: [],
            entry_count: 1,
            sensitive_entry_count: 1,
            total_value_size_bytes: 317
          }
        ]
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryTree.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        capabilities: {
          list_entries: true,
          list_tree: true,
          search_entries: true,
          reveal_value: true
        },
        supported: true,
        inspection_path: [],
        nodes: [
          {
            node_ref: 'root-session-node',
            label: '00000000-0000-0000-0000-000000000001',
            inspection_path: ['00000000-0000-0000-0000-000000000001'],
            depth: 1,
            has_children: false
          }
        ],
        next_cursor: null,
        limit: 50,
        byte_limit: 65536,
        emitted_bytes: 0,
        truncated_by_byte_limit: false
      }
    );
    hostInfrastructureApi.fetchSettingsHostInfrastructureMemoryEntries.mockResolvedValue(
      {
        contract_code: 'session-store',
        label: 'Sessions',
        provider_code: 'local',
        capabilities: {
          list_entries: true,
          list_tree: true,
          search_entries: true,
          reveal_value: true
        },
        supported: true,
        inspection_path: ['00000000-0000-0000-0000-000000000001'],
        entries: [
          {
            contract_code: 'session-store',
            group_code: '00000000-0000-0000-0000-000000000001',
            entry_ref: 'session:1',
            key: 'session:1',
            inspection_path: [
              '00000000-0000-0000-0000-000000000001',
              'session:1'
            ],
            entry_kind: 'session',
            status: 'active',
            owner: 'user-1',
            value_size_bytes: 317,
            metadata_size_bytes: 2,
            ttl_seconds: 600,
            created_at_unix: 1_700_000_000,
            expires_at_unix: 1_700_000_600,
            sensitive: true,
            metadata: {}
          }
        ],
        next_cursor: null,
        limit: 50,
        byte_limit: 65536,
        emitted_bytes: 128,
        truncated_by_byte_limit: false
      }
    );

    renderApp('/settings/memory-observation');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/memory-observation');
    });
    expect(
      await screen.findByRole('link', { name: '内存观察' }, { timeout: 10000 })
    ).toHaveAttribute('href', '/settings/memory-observation');
    expect(
      await screen.findByRole('tab', { name: 'Sessions' }, { timeout: 10000 })
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('tab', { name: '统计' }, { timeout: 10000 })
    ).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('tab', { name: 'Sessions' }));
    fireEvent.click(
      await screen.findByText('00000000-0000-0000-0000-000000000001')
    );
    expect(await screen.findByText('session:1')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Provider 配置' })
    ).not.toBeInTheDocument();
  }, 10000);

  test('shows 文件管理 when file_table.view.own is present', async () => {
    authenticateWithPermissions(['route_page.view.all', 'file_table.view.own']);

    renderApp('/settings/files');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/files');
    });
    expect(
      await screen.findByRole('tab', { name: '文件表' })
    ).toBeInTheDocument();
  });

  test('uses API key as the baseline settings section', async () => {
    authenticateWithPermissions(['route_page.view.all']);

    renderApp('/settings');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(
      await screen.findByRole('button', { name: /添加/ })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('navigation', { name: 'Section navigation' })
    ).toBeInTheDocument();
  });
});
