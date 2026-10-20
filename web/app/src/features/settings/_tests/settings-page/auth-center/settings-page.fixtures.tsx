import { act, render, waitFor, within } from '@testing-library/react';
import { Grid } from 'antd';
import { beforeAll, beforeEach, expect, vi } from 'vitest';

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
  updateSettingsAuthCenterLoginEntryEnabled: vi.fn(),
  updateSettingsAuthCenterLoginEntryConfig: vi.fn(),
  updateSettingsAuthCenterLoginEntryPublicUiBlock: vi.fn(),
  createSettingsAuthCenterLoginEntry: vi.fn(),
  copySettingsAuthCenterLoginEntry: vi.fn(),
  deleteSettingsAuthCenterLoginEntry: vi.fn(),
  reorderSettingsAuthCenterLoginEntries: vi.fn()
}));

const authenticatorConfigSchema = () => [
  {
    key: 'title',
    label: 'Authenticator title',
    type: 'string' as const,
    required: true
  },
  {
    key: 'description',
    label: 'Description',
    type: 'string' as const,
    control: 'textarea' as const
  },
  {
    key: 'enabled',
    label: 'Enabled',
    type: 'boolean' as const,
    control: 'switch' as const
  },
  {
    key: 'self_registration_enabled',
    label: 'Allow self registration',
    type: 'boolean' as const,
    control: 'switch' as const
  }
];

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
  settingsSystemRuntimeProcessesQueryKey: [
    'settings',
    'system-runtime',
    'processes'
  ],
  fetchSettingsSystemRuntimeProfile: vi.fn(),
  fetchSettingsSystemRuntimeProcesses: vi.fn(),
  terminateSettingsSystemRuntimeProcess: vi.fn()
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
  fetchSettingsHostInfrastructureMemoryOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStatsOverview: vi.fn(),
  fetchSettingsHostInfrastructureMemoryStats: vi.fn(),
  fetchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  fetchSettingsHostInfrastructureMemoryTree: vi.fn(),
  searchSettingsHostInfrastructureMemoryEntries: vi.fn(),
  revealSettingsHostInfrastructureMemoryEntry: vi.fn()
}));

const dataModelsApi = vi.hoisted(() => ({
  settingsDataSourcesQueryKey: ['settings', 'data-models', 'data-sources'],
  settingsDataSourceCatalogQueryKey: [
    'settings',
    'data-models',
    'data-source-catalog'
  ],
  settingsDataSourceResourcesQueryKey: vi.fn((dataSourceId: string) => [
    'settings',
    'data-models',
    'data-sources',
    dataSourceId,
    'resources'
  ]),
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
  fetchSettingsDataSources: vi.fn(),
  fetchSettingsDataSourceCatalog: vi.fn(),
  createSettingsDataSource: vi.fn(),
  validateSettingsDataSource: vi.fn(),
  fetchSettingsDataSourceResources: vi.fn(),
  discoverSettingsDataSourceResources: vi.fn(),
  previewSettingsDataSourceResource: vi.fn(),
  mapSettingsDataSourceResourceToModel: vi.fn(),
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

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock('../../../api/identity-access/members', () => membersApi);
vi.mock('../../../api/identity-access/roles', () => rolesApi);
vi.mock('../../../api/identity-access/permissions', () => permissionsApi);
vi.mock('../../../api/api-docs', () => docsApi);
vi.mock(
  '../../../api/identity-access/personal-access-tokens',
  () => personalAccessTokensApi
);
vi.mock('../../../api/identity-access/auth-center', () => authCenterApi);
vi.mock('../../../api/model-providers', () => modelProvidersApi);
vi.mock('../../../api/extensions/plugins', () => pluginsApi);
vi.mock('../../../api/system-runtime', () => systemRuntimeApi);
vi.mock('../../../api/file-management', () => fileManagementApi);
vi.mock('../../../api/host-infrastructure', () => hostInfrastructureApi);
vi.mock('../../../api/data-models', () => dataModelsApi);
vi.mock('../../../api/console-navigation', () => consoleNavigationApi);
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
  RadarComponent: {},
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
    listConsoleMemberRoleOptions: rolesApi.fetchSettingsRoles,
    getConsoleDepartmentAccess: vi.fn().mockResolvedValue({
      can_list: false,
      can_create: false,
      can_update: false,
      can_delete: false,
      can_replace_member_departments: false,
      can_assign_roles: true
    }),
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

import { AppProviders } from '../../../../../app/AppProviders';
import { AppRouterProvider } from '../../../../../app/router';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import {
  settingsSectionDefinitions,
  type SettingsSectionKey
} from '../../../lib/settings-sections';

// Warm the whole real lazy route chain before UI assertions start. Preloading
// only the section leaves the shell and settings owner cold on the first render.
beforeAll(async () => {
  await Promise.all([
    import('../../../../../app-shell/AppShellFrame'),
    import('../../../pages/SettingsPage'),
    import('../../../pages/settings-page/SettingsAuthCenterSection')
  ]);
});

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');

function settingsConsoleNavigation(
  sectionKeys: SettingsSectionKey[] = settingsSectionDefinitions.map(
    (section) => section.key
  )
) {
  const sections = sectionKeys.flatMap((sectionKey) => {
    const section = settingsSectionDefinitions.find(
      (definition) => definition.key === sectionKey
    );

    return section ? [section] : [];
  });

  return {
    route_definitions: sections.map((section) => ({
      route_id: `settings.${section.key}`,
      surface_key: section.key,
      path: section.to,
      surface_kind: 'system' as const
    })),
    navigation_items: sections.map((section, index) => ({
      item_id: section.key,
      route_id: `settings.${section.key}`,
      parent_item_id: 'settings',
      label_key: section.label_key,
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

async function renderAuthCenterReady() {
  const navigationBaseline =
    consoleNavigationApi.fetchSettingsConsoleNavigation.mock.calls.length;
  const overviewBaseline =
    authCenterApi.fetchSettingsAuthCenterOverview.mock.calls.length;
  const navigation = settingsConsoleNavigation();
  let resolveNavigation!: (value: typeof navigation) => void;
  const pendingNavigation = new Promise<typeof navigation>((resolve) => {
    resolveNavigation = resolve;
  });
  consoleNavigationApi.fetchSettingsConsoleNavigation.mockReturnValue(
    pendingNavigation
  );

  const context = (phase: string) =>
    JSON.stringify({
      phase,
      pathname: window.location.pathname,
      navigationBaseline,
      navigationCalls:
        consoleNavigationApi.fetchSettingsConsoleNavigation.mock.calls.length,
      overviewBaseline,
      overviewCalls:
        authCenterApi.fetchSettingsAuthCenterOverview.mock.calls.length,
      overviewResultTypes:
        authCenterApi.fetchSettingsAuthCenterOverview.mock.results
          .slice(overviewBaseline, overviewBaseline + 3)
          .map((result) => result.type),
      main: document
        .querySelector('.app-shell-content')
        ?.outerHTML.slice(0, 2000)
    });
  const main = (phase: string) => {
    const element = document.querySelector<HTMLElement>('.app-shell-content');
    if (!element) throw new Error(context(phase));
    return element;
  };

  const waitForPhase = (phase: string, assertion: () => void) =>
    waitFor(assertion, {
      onTimeout: (error) => new Error(context(phase), { cause: error })
    });

  const view = renderApp('/settings/auth-center');
  await waitForPhase('navigation-pending', () => {
    const pendingContext = context('navigation-pending');
    expect(
      consoleNavigationApi.fetchSettingsConsoleNavigation.mock.calls.length,
      pendingContext
    ).toBeGreaterThan(navigationBaseline);
    expect(
      within(main('navigation-pending')).getByTestId('section-page-layout'),
      pendingContext
    ).toBeInTheDocument();
    expect(window.location.pathname, pendingContext).toBe(
      '/settings/auth-center'
    );
    expect(
      authCenterApi.fetchSettingsAuthCenterOverview.mock.calls.length,
      pendingContext
    ).toBe(overviewBaseline);
  });

  await act(async () => {
    resolveNavigation(navigation);
  });

  await waitForPhase('overview-requested', () => {
    const requestedContext = context('overview-requested');
    expect(
      authCenterApi.fetchSettingsAuthCenterOverview.mock.calls.length,
      requestedContext
    ).toBeGreaterThan(overviewBaseline);
    expect(
      authCenterApi.fetchSettingsAuthCenterOverview.mock.results[
        overviewBaseline
      ]?.type,
      requestedContext
    ).toBe('return');
  });
  const overviewResult =
    authCenterApi.fetchSettingsAuthCenterOverview.mock.results[
      overviewBaseline
    ];
  if (overviewResult.type !== 'return') {
    throw new Error(context('overview-result'));
  }
  // Observe the application's actual request; do not consume a queued mock response.
  try {
    await act(async () => {
      await overviewResult.value;
    });
  } catch (error) {
    throw new Error(context('overview-resolved'), { cause: error });
  }

  await waitForPhase('auth-table-ready', () => {
    const tableContext = context('auth-table-ready');
    const table = within(main('auth-table-ready')).getByRole('table');
    expect(
      table.closest('.settings-auth-center__table'),
      tableContext
    ).not.toBeNull();
  });
  return view;
}

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
  consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
  consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
    settingsConsoleNavigation()
  );
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
  personalAccessTokensApi.createSettingsPersonalAccessToken.mockResolvedValue({
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
  });
  personalAccessTokensApi.revokeSettingsPersonalAccessToken.mockResolvedValue(
    undefined
  );
  authCenterApi.fetchSettingsAuthCenterOverview.mockResolvedValue({
    default_login_entry_id: 'auth-password-local',
    supported_auth_types: ['password-local'],
    authentication_provider_templates: [
      { auth_type: 'password-local', display_name: 'Password' }
    ],
    login_entries: [
      {
        id: 'auth-password-local',
        auth_type: 'password-local',
        title: 'Password',
        enabled: true,
        is_builtin: true,
        sort_order: 0,
        config_schema: authenticatorConfigSchema(),
        config_values: {
          title: 'Password',
          enabled: true,
          description: null,
          extension_config: {}
        }
      }
    ]
  });
  authCenterApi.updateSettingsAuthCenterLoginEntryEnabled.mockResolvedValue(
    undefined
  );
  authCenterApi.updateSettingsAuthCenterLoginEntryConfig.mockResolvedValue(
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
    related_process_memory_complete: true,
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
        service: 'runtime-extension-host',
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
        related_process_bytes: 1610612736,
        related_process_count: 2,
        memory: {
          total_bytes: 17179869184,
          total_gb: 16,
          available_bytes: 8589934592,
          available_gb: 8,
          process_bytes: 1073741824,
          process_gb: 1
        },
        services: ['api-server', 'runtime-extension-host']
      }
    ],
    runtime_targets: [
      {
        target_id: 'api-server',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: {
          captured_at_unix_milliseconds: 1_752_745_600_000,
          sample_interval_milliseconds: 2000,
          cpu: {
            availability: 'available',
            scope_kind: 'host',
            usage_percent: 12.5,
            logical_count: 8,
            limit_cores: 8
          },
          memory: {
            availability: 'available',
            scope_kind: 'host',
            total_bytes: 17179869184,
            available_bytes: 8589934592,
            used_bytes: 8589934592,
            process_bytes: 1073741824,
            related_process_bytes: 1073741824,
            related_process_count: 1,
            cgroup_composition: null
          },
          storage: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            mount_point: '/',
            file_system: 'overlay',
            total_bytes: 68719476736,
            available_bytes: 51539607552,
            used_bytes: 17179869184
          },
          network: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            received_bytes_per_second: 2048,
            transmitted_bytes_per_second: 1024
          },
          disk_io: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            read_bytes_per_second: 4096,
            written_bytes_per_second: 8192
          }
        }
      },
      {
        target_id: 'runtime-extension-host',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: {
          captured_at_unix_milliseconds: 1_752_745_600_000,
          sample_interval_milliseconds: 2000,
          cpu: {
            availability: 'available',
            scope_kind: 'host',
            usage_percent: 8.5,
            logical_count: 8,
            limit_cores: 8
          },
          memory: {
            availability: 'available',
            scope_kind: 'host',
            total_bytes: 17179869184,
            available_bytes: 8589934592,
            used_bytes: 8589934592,
            process_bytes: 536870912,
            related_process_bytes: 536870912,
            related_process_count: 1,
            cgroup_composition: null
          },
          storage: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            mount_point: '/',
            file_system: 'overlay',
            total_bytes: 68719476736,
            available_bytes: 51539607552,
            used_bytes: 17179869184
          },
          network: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            received_bytes_per_second: 1024,
            transmitted_bytes_per_second: 512
          },
          disk_io: {
            availability: 'available',
            scope_kind: 'runtime_visible',
            read_bytes_per_second: 2048,
            written_bytes_per_second: 4096
          }
        }
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
  dataModelsApi.fetchSettingsDataSources.mockResolvedValue([
    {
      id: 'main',
      display_name: '主数据源',
      status: 'ready',
      enabled: true,
      fixed: true,
      default_data_model_status: 'published',
      capabilities: {
        can_update_defaults: true,
        can_create_data_model: true,
        can_validate: false,
        can_discover_resources: false,
        can_preview_resources: false,
        can_map_resources: false
      },
      backend: { kind: 'core', durable_backend: 'postgresql' }
    }
  ]);
  dataModelsApi.fetchSettingsDataSourceCatalog.mockResolvedValue({
    entries: []
  });
  dataModelsApi.fetchSettingsDataModels.mockResolvedValue([]);
  dataModelsApi.fetchSettingsDataModelScopeGrants.mockResolvedValue([]);
  dataModelsApi.fetchSettingsDataModelAdvisorFindings.mockResolvedValue([]);
  dataModelsApi.fetchSettingsDataModelRecordPreview.mockResolvedValue(null);
});

export {
  authCenterApi,
  authenticateWithPermissions,
  authenticatorConfigSchema,
  consoleNavigationApi,
  docsApi,
  renderApp,
  renderAuthCenterReady,
  settingsConsoleNavigation
};
