import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { Grid } from 'antd';
import { createElement } from 'react';
import { expect, vi } from 'vitest';

import type {
  SettingsDataModel,
  SettingsDataModelField
} from '../../api/data-models';

export const SLOW_SETTINGS_PAGE_TEST_TIMEOUT = 20_000;

vi.setConfig({ testTimeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT });

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
  settingsApiDocsCategoryOperationsQueryKey: vi.fn(),
  settingsApiDocsOperationSpecQueryKey: vi.fn(),
  fetchSettingsApiDocsCatalog: vi.fn(),
  fetchSettingsApiDocsCategoryOperations: vi.fn(),
  fetchSettingsApiOperationSpec: vi.fn()
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
  settingsModelProviderModelsQueryKey: vi.fn(),
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

const hostInfrastructureApi = vi.hoisted(() => ({
  settingsHostInfrastructureProvidersQueryKey: [
    'settings',
    'host-infrastructure',
    'providers'
  ],
  fetchSettingsHostInfrastructureProviders: vi.fn(),
  saveSettingsHostInfrastructureProviderConfig: vi.fn()
}));

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
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
  settingsDataModelOpenApiQueryKey: vi.fn((modelId: string) => [
    'settings',
    'data-models',
    'openapi',
    modelId
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
  fetchSettingsDataModelRecordPreview: vi.fn(),
  fetchSettingsDataModelOpenApiDocument: vi.fn()
}));

vi.mock('../../api/members', () => membersApi);
vi.mock('../../api/roles', () => rolesApi);
vi.mock('../../api/permissions', () => permissionsApi);
vi.mock('../../api/api-docs', () => docsApi);
vi.mock('../../api/model-providers', () => modelProvidersApi);
vi.mock('../../api/plugins', () => pluginsApi);
vi.mock('../../api/system-runtime', () => systemRuntimeApi);
vi.mock('../../api/file-management', () => fileManagementApi);
vi.mock('../../api/host-infrastructure', () => hostInfrastructureApi);
vi.mock('../../api/data-models', () => dataModelsApi);
vi.mock('../../api/console-navigation', () => consoleNavigationApi);
vi.mock('@scalar/api-reference-react', () => ({
  ApiReferenceReact: () =>
    createElement('div', { 'data-testid': 'settings-page-scalar' }, 'Scalar')
}));

import { AppProviders } from '../../../../app/AppProviders';
import { AppRouterProvider } from '../../../../app/router';
import { i18nText } from '../../../../shared/i18n/text';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { DataModelFormDrawer } from '../../components/data-models/DataModelFormDrawer';

const useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint');
const antdStaticMessageWarning =
  'Static function can not consume context like dynamic theme';
const consoleWarn = console.warn;
const consoleError = console.error;
let consoleWarnSpy: ReturnType<typeof vi.spyOn>;
let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

const manageableModelCapabilities: SettingsDataModel['capabilities'] = {
  can_delete: true,
  can_add_user_field: true,
  can_update_lifecycle_status: true,
  record: {
    can_list: true,
    can_get: true,
    can_create: true,
    can_update: true,
    can_delete: true
  }
};

const protectedBuiltinModelCapabilities: SettingsDataModel['capabilities'] = {
  can_delete: false,
  can_add_user_field: true,
  can_update_lifecycle_status: false,
  record: {
    can_list: true,
    can_get: true,
    can_create: true,
    can_update: true,
    can_delete: true
  }
};

const readOnlyRuntimeModelCapabilities: SettingsDataModel['capabilities'] = {
  can_delete: false,
  can_add_user_field: false,
  can_update_lifecycle_status: false,
  record: {
    can_list: true,
    can_get: true,
    can_create: false,
    can_update: false,
    can_delete: false
  }
};

const manageableFieldCapabilities: SettingsDataModelField['capabilities'] = {
  ownership: 'user_added',
  can_update_presentation_metadata: true,
  can_update_physical_metadata: true,
  can_delete: true
};

const systemFieldCapabilities: SettingsDataModelField['capabilities'] = {
  ownership: 'system_owned',
  can_update_presentation_metadata: true,
  can_update_physical_metadata: false,
  can_delete: false
};

function authenticate() {
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
      permissions: [
        'state_model.view.all',
        'state_model.manage.all',
        'api_reference.view.all'
      ]
    }
  });
}

export function renderApp(pathname: string) {
  window.history.pushState({}, '', pathname);

  return render(
    createElement(AppProviders, null, createElement(AppRouterProvider))
  );
}

export function findDataModelsNavigation() {
  return screen.findByRole('link', { name: '数据源' }, { timeout: 5000 });
}

export async function openContactsDataModelEditor() {
  await screen.findByText(
    'Contacts',
    {},
    { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
  );
  return openDataModelEditorByTitle('Contacts');
}

export async function openDataModelEditorByTitle(title: string) {
  await screen.findByText(
    title,
    {},
    { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT }
  );
  const contactsRow = screen
    .getAllByRole('row')
    .find((row) => within(row).queryByText(title));
  expect(contactsRow).toBeInstanceOf(HTMLElement);

  fireEvent.click(
    within(contactsRow as HTMLElement).getByRole('button', { name: '编辑' })
  );

  expect(await screen.findByText(`编辑 ${title}`)).toBeInTheDocument();
  return screen.findByRole('region', { name: 'Data Model 详情' });
}

function settingsDataModelField(
  id: string,
  code: string,
  title: string,
  fieldKind = 'string',
  overrides: Partial<SettingsDataModelField> = {}
): SettingsDataModelField {
  return {
    id,
    code,
    title,
    description: null,
    physical_column_name: code,
    external_field_key: null,
    field_kind: fieldKind,
    is_system: false,
    is_writable: true,
    is_required: false,
    api_required: false,
    is_unique: false,
    default_value: null,
    display_interface: 'input',
    display_options: {},
    relation_target_model_id: null,
    relation_options: {},
    sort_order: 0,
    capabilities: manageableFieldCapabilities,
    ...overrides
  };
}

function systemSettingsDataModelField(
  id: string,
  code: string,
  title: string,
  fieldKind = 'string',
  overrides: Partial<SettingsDataModelField> = {}
): SettingsDataModelField {
  return settingsDataModelField(id, code, title, fieldKind, {
    capabilities: systemFieldCapabilities,
    ...overrides
  });
}

function settingsDataModel(
  id: string,
  code: string,
  title: string,
  fields: ReturnType<typeof settingsDataModelField>[],
  overrides: Partial<SettingsDataModel> = {}
): SettingsDataModel {
  return {
    id,
    scope_kind: 'system',
    scope_id: '00000000-0000-0000-0000-000000000000',
    code,
    title,
    status: 'published',
    runtime_availability: 'available',
    data_source_id: 'main',
    source_kind: 'main_source',
    external_resource_key: null,
    external_table_id: null,
    physical_table_name: `dm_${code}`,
    acl_namespace: `data_model.${code}`,
    audit_namespace: `data_model.${code}`,
    fields,
    builtin_kind: null,
    capabilities: manageableModelCapabilities,
    ...overrides
  };
}

export const contactsModel = settingsDataModel(
  'model-1',
  'contacts',
  'Contacts',
  [
    settingsDataModelField('field-1', 'email', 'Email', 'string', {
      description: 'Primary contact email',
      external_field_key: 'email',
      is_required: true,
      api_required: true,
      is_unique: true
    }),
    settingsDataModelField('field-2', 'first_name', 'First name', 'string', {
      external_field_key: 'first_name'
    }),
    systemSettingsDataModelField(
      'field-remote-id',
      'remote_id',
      'Remote ID',
      'string',
      {
        external_field_key: 'remote_id',
        is_system: true,
        is_writable: false
      }
    )
  ],
  {
    scope_kind: 'workspace',
    scope_id: 'workspace-1',
    data_source_id: 'source-1',
    source_kind: 'external_source',
    external_resource_key: 'contacts',
    external_table_id: 'crm.contacts',
    physical_table_name: 'dm_contacts'
  }
);

const draftOrdersModel = settingsDataModel(
  'model-draft-orders',
  'draft_orders',
  'Draft Orders',
  [settingsDataModelField('draft-order-title', 'title', 'Title')],
  {
    scope_kind: 'workspace',
    scope_id: 'workspace-1',
    status: 'draft',
    runtime_availability: 'unavailable',
    data_source_id: 'source-1',
    source_kind: 'external_source',
    external_resource_key: 'draft_orders',
    external_table_id: 'crm.draft_orders',
    physical_table_name: 'dm_draft_orders'
  }
);

const mainSourceModels = [
  settingsDataModel(
    'model-attachments',
    'attachments',
    'Attachments',
    [
      systemSettingsDataModelField(
        'attachment-name',
        'name',
        '文件名',
        'string',
        {
          description: '附件原始文件名',
          is_required: true
        }
      ),
      systemSettingsDataModelField(
        'attachment-size',
        'size',
        '文件大小',
        'number',
        {
          is_required: true
        }
      )
    ],
    {
      builtin_kind: 'core',
      capabilities: protectedBuiltinModelCapabilities
    }
  ),
  settingsDataModel(
    'model-users',
    'users',
    '用户',
    [
      systemSettingsDataModelField(
        'user-username',
        'username',
        '用户名',
        'string',
        {
          is_required: true,
          is_unique: true
        }
      ),
      systemSettingsDataModelField(
        'user-display-name',
        'display_name',
        '显示名称',
        'string',
        {
          is_required: true
        }
      ),
      systemSettingsDataModelField('user-email', 'email', '邮箱', 'string', {
        is_unique: true
      }),
      systemSettingsDataModelField('user-status', 'status', '状态', 'string', {
        is_required: true
      }),
      systemSettingsDataModelField(
        'user-role-codes',
        'role_codes',
        '角色',
        'json'
      ),
      systemSettingsDataModelField(
        'user-created-at',
        'created_time',
        '创建时间',
        'datetime',
        {
          is_required: true
        }
      ),
      systemSettingsDataModelField(
        'user-last-login-at',
        'last_login_at',
        '最后登录时间',
        'datetime'
      )
    ],
    {
      builtin_kind: 'core',
      capabilities: protectedBuiltinModelCapabilities
    }
  ),
  settingsDataModel(
    'model-roles',
    'roles',
    '角色',
    [
      systemSettingsDataModelField('role-code', 'code', '角色标识', 'string', {
        is_required: true,
        is_unique: true
      }),
      systemSettingsDataModelField('role-name', 'name', '角色名称', 'string', {
        is_required: true
      }),
      systemSettingsDataModelField(
        'role-scope-kind',
        'scope_kind',
        '作用域',
        'string',
        {
          is_required: true
        }
      ),
      systemSettingsDataModelField(
        'role-builtin',
        'is_builtin',
        '内置角色',
        'boolean',
        {
          is_required: true
        }
      ),
      systemSettingsDataModelField(
        'role-default-member',
        'is_default_member_role',
        '默认成员角色',
        'boolean',
        { is_required: true }
      ),
      systemSettingsDataModelField(
        'role-created-at',
        'created_time',
        '创建时间',
        'datetime',
        {
          is_required: true
        }
      )
    ],
    {
      builtin_kind: 'core',
      capabilities: protectedBuiltinModelCapabilities
    }
  ),
  settingsDataModel(
    'model-runtime-logs',
    'runtime_logs',
    'Runtime Logs',
    [
      systemSettingsDataModelField(
        'runtime-log-level',
        'level',
        'Level',
        'string',
        {
          description: 'Runtime log level'
        }
      )
    ],
    {
      builtin_kind: 'runtime_read',
      capabilities: readOnlyRuntimeModelCapabilities
    }
  )
];

export function setupDataModelsPageTest() {
  consoleWarnSpy = vi
    .spyOn(console, 'warn')
    .mockImplementation((...args) => consoleWarn(...args));
  consoleErrorSpy = vi
    .spyOn(console, 'error')
    .mockImplementation((...args) => consoleError(...args));

  resetAuthStore();
  authenticate();
  useBreakpointSpy.mockReturnValue({
    xs: true,
    sm: true,
    md: true,
    lg: true,
    xl: false,
    xxl: false
  });

  consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
    route_definitions: [
      {
        route_id: 'settings.data-models',
        surface_key: 'data-models',
        path: '/settings/data-models',
        surface_kind: 'system'
      }
    ],
    navigation_items: [
      {
        item_id: 'data-models',
        route_id: 'settings.data-models',
        parent_item_id: 'settings',
        label_key: 'auto.data_source',
        navigation_slot: 'settings',
        order: 1
      }
    ],
    permission_bindings: []
  });

  membersApi.fetchSettingsMembers.mockResolvedValue([]);
  rolesApi.fetchSettingsRoles.mockResolvedValue([]);
  rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
    role_code: 'root',
    permission_codes: []
  });
  permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
  docsApi.fetchSettingsApiDocsCatalog.mockResolvedValue({
    title: '1flowbase API',
    version: '0.1.0',
    categories: []
  });
  modelProvidersApi.fetchSettingsModelProviderCatalog.mockResolvedValue([]);
  modelProvidersApi.fetchSettingsModelProviderInstances.mockResolvedValue([]);
  modelProvidersApi.fetchSettingsModelProviderOptions.mockResolvedValue({
    providers: []
  });
  pluginsApi.fetchSettingsPluginFamilies.mockResolvedValue({
    locale_meta: {},
    i18n_catalog: {},
    entries: []
  });
  pluginsApi.fetchSettingsOfficialPluginCatalog.mockResolvedValue({
    source_kind: 'official_registry',
    source_label: '官方源',
    registry_url: 'https://official.example.com/official-registry.json',
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
  hostInfrastructureApi.fetchSettingsHostInfrastructureProviders.mockResolvedValue(
    []
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
    },
    {
      id: 'source-1',
      display_name: 'HubSpot',
      status: 'ready',
      enabled: true,
      fixed: false,
      default_data_model_status: 'draft',
      capabilities: {
        can_update_defaults: true,
        can_create_data_model: false,
        can_validate: true,
        can_discover_resources: true,
        can_preview_resources: true,
        can_map_resources: true
      },
      backend: {
        kind: 'runtime_extension',
        installation_id: 'installation-1',
        source_code: 'hubspot',
        config_json: {},
        secret_ref: null,
        secret_version: null,
        catalog_refresh_status: 'ready',
        catalog_last_error_message: null,
        catalog_refreshed_at: '2026-04-30T08:00:00Z'
      }
    }
  ]);
  dataModelsApi.fetchSettingsDataSourceCatalog.mockResolvedValue({
    entries: [
      {
        installation_id: 'installation-1',
        source_code: 'hubspot',
        plugin_id: 'hubspot@1.0.0',
        plugin_version: '1.0.0',
        display_name: 'HubSpot',
        protocol: 'stdio_json',
        config_schema: []
      }
    ]
  });
  dataModelsApi.fetchSettingsDataSourceResources.mockResolvedValue({
    entries: [
      {
        resource_key: 'contacts',
        display_name: 'Contacts',
        resource_kind: 'object',
        capabilities: {},
        metadata: {}
      }
    ],
    refresh_status: 'ready',
    last_error_message: null,
    refreshed_at: '2026-04-30T08:00:00Z'
  });
  dataModelsApi.fetchSettingsDataModels.mockImplementation((dataSourceId: string) =>
    Promise.resolve(
      dataSourceId === 'main'
        ? mainSourceModels
        : [contactsModel, draftOrdersModel]
    )
  );
  dataModelsApi.fetchSettingsDataModelScopeGrants.mockResolvedValue([
    {
      id: 'grant-owner',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      data_model_id: 'model-1',
      enabled: true,
      permission_profile: 'owner'
    },
    {
      id: 'grant-scope',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      data_model_id: 'model-1',
      enabled: true,
      permission_profile: 'scope_all'
    },
    {
      id: 'grant-system',
      scope_kind: 'system',
      scope_id: '00000000-0000-0000-0000-000000000000',
      data_model_id: 'model-1',
      enabled: false,
      permission_profile: 'system_all'
    }
  ]);
  dataModelsApi.fetchSettingsDataModelAdvisorFindings.mockResolvedValue([
    {
      id: 'finding-1',
      data_model_id: 'model-1',
      severity: 'blocking',
      code: 'unsafe_external_source',
      message: 'External source needs scope filtering.',
      recommended_action: 'Enable scope filtering.',
      can_acknowledge: false
    },
    {
      id: 'finding-2',
      data_model_id: 'model-1',
      severity: 'high',
      code: 'protected_model_exposure_attempt',
      message: 'Protected model needs review.',
      recommended_action: 'Review protection settings.',
      can_acknowledge: false
    },
    {
      id: 'finding-3',
      data_model_id: 'model-1',
      severity: 'info',
      code: 'field_mapping_notice',
      message: 'Field mapping is informational.',
      recommended_action: 'Review field mapping.',
      can_acknowledge: true
    }
  ]);
  dataModelsApi.fetchSettingsDataModelRecordPreview.mockResolvedValue({
    items: [
      {
        id: 'record-1',
        email: 'person@example.com'
      }
    ],
    total: 1
  });
  dataModelsApi.fetchSettingsDataModelOpenApiDocument.mockResolvedValue({
    openapi: '3.1.0',
    info: {
      title: 'Contacts Data Model API',
      version: '1.0.0'
    },
    paths: {},
    components: {
      schemas: {
        ContactsRecord: {
          type: 'object',
          required: ['email', 'remote_id'],
          properties: {
            email: {
              type: 'string',
              title: 'Email'
            },
            first_name: {
              type: 'string',
              title: 'First name'
            },
            remote_id: {
              type: 'string',
              title: 'Remote ID',
              readOnly: true
            }
          }
        },
        ContactsRecordCreateInput: {
          type: 'object',
          required: ['email'],
          properties: {
            email: {
              type: 'string',
              title: 'Email'
            },
            first_name: {
              type: 'string',
              title: 'First name'
            }
          }
        },
        ContactsRecordUpdateInput: {
          type: 'object',
          properties: {
            email: {
              type: 'string',
              title: 'Email'
            },
            first_name: {
              type: 'string',
              title: 'First name'
            }
          }
        }
      }
    }
  });
  dataModelsApi.updateSettingsDataModel.mockResolvedValue({
    id: 'model-1'
  });
  dataModelsApi.deleteSettingsDataModel.mockResolvedValue({
    deleted: true
  });
  dataModelsApi.createSettingsDataModel.mockResolvedValue({
    id: 'model-new'
  });
  dataModelsApi.createSettingsDataSource.mockResolvedValue({
    id: 'source-new'
  });
  dataModelsApi.validateSettingsDataSource.mockResolvedValue({
    data_source: { id: 'source-1', status: 'ready' },
    output: {}
  });
  dataModelsApi.discoverSettingsDataSourceResources.mockResolvedValue({
    entries: []
  });
  dataModelsApi.previewSettingsDataSourceResource.mockResolvedValue({
    output: { rows: [], next_cursor: null }
  });
  dataModelsApi.mapSettingsDataSourceResourceToModel.mockResolvedValue({
    id: 'model-mapped'
  });
  dataModelsApi.updateSettingsDataSourceDefaults.mockResolvedValue({});
  dataModelsApi.createSettingsDataModelField.mockResolvedValue({
    id: 'field-new'
  });
  dataModelsApi.updateSettingsDataModelField.mockResolvedValue({
    id: 'field-1'
  });
  dataModelsApi.deleteSettingsDataModelField.mockResolvedValue({
    deleted: true
  });
  dataModelsApi.updateSettingsDataModelScopeGrant.mockResolvedValue({
    id: 'grant-owner'
  });
  dataModelsApi.createSettingsDataModelScopeGrant.mockResolvedValue({
    id: 'grant-new'
  });
}

export function cleanupDataModelsPageTest() {
  const warningCalls = [
    ...consoleWarnSpy.mock.calls,
    ...consoleErrorSpy.mock.calls
  ].filter((args) =>
    args.some((arg: unknown) => String(arg).includes(antdStaticMessageWarning))
  );

  consoleWarnSpy.mockRestore();
  consoleErrorSpy.mockRestore();

  expect(warningCalls).toEqual([]);
}

export {
  DataModelFormDrawer,
  dataModelsApi,
  fireEvent,
  i18nText,
  render,
  screen,
  waitFor,
  within
};
