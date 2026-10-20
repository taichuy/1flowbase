import { beforeEach, vi } from 'vitest';

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
  settingsConsolePolicyCatalogQueryKey: vi.fn((locale: string) => [
    'settings',
    'console-policy-catalog',
    locale
  ]),
  fetchSettingsConsolePolicyCatalog: vi.fn(),
  replaceSettingsConsolePolicyOrder: vi.fn()
}));

const dataModelsApi = vi.hoisted(() => ({
  settingsAllDataModelsQueryKey: ['settings', 'data-models', 'models', 'all'],
  fetchSettingsAllDataModels: vi.fn()
}));

vi.mock('../../api/identity-access/roles', () => rolesApi);
vi.mock('../../api/identity-access/permissions', () => permissionsApi);
vi.mock('../../api/data-models', () => dataModelsApi);

import { resetAuthStore } from '../../../../state/auth-store';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import {
  authenticate,
  renderRolePermissionPanel,
  defaultDataPolicy,
  consolePolicyCatalog,
  defaultDataModels
} from './fixtures';

let RolePermissionPanel: typeof import('../../components/RolePermissionPanel').RolePermissionPanel;

function renderPanel(canManageRoles = true) {
  return renderRolePermissionPanel(RolePermissionPanel, canManageRoles);
}

beforeEach(async () => {
  vi.clearAllMocks();
  await loadApplicationI18nResources();
  ({ RolePermissionPanel } =
    await import('../../components/RolePermissionPanel'));
  await appI18n.changeLanguage('zh_Hans');
  resetAuthStore();
  authenticate();
  rolesApi.fetchSettingsRoles.mockResolvedValue([
    {
      code: 'member',
      name: 'Member',
      introduction: '默认管理角色',
      scope_kind: 'workspace',
      is_builtin: false,
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
    tree: [
      {
        id: 'root-page',
        kind: 'group',
        title: '工作台',
        slug: 'pgr3083h',
        children: [
          {
            id: 'child-page',
            kind: 'page',
            title: '页面一',
            slug: null,
            children: [
              {
                id: 'child-tab',
                kind: 'tab',
                title: '标签一',
                slug: null,
                children: []
              }
            ]
          }
        ]
      }
    ],
    checked_page_ids: [],
    checked_tab_ids: []
  });
  rolesApi.replaceSettingsRoleFrontstageRoutes.mockResolvedValue(undefined);
  rolesApi.createSettingsRole.mockResolvedValue({
    code: 'qa',
    name: 'QA',
    introduction: '测试角色',
    scope_kind: 'workspace',
    is_builtin: false,
    is_editable: true,
    auto_grant_new_permissions: true,
    is_default_member_role: false,
    permission_codes: []
  });
  rolesApi.updateSettingsRole.mockResolvedValue(undefined);
  rolesApi.fetchSettingsRoleDataPolicy.mockResolvedValue(defaultDataPolicy);
  rolesApi.replaceSettingsRoleDataPolicy.mockImplementation(
    async (role_code, input) => ({ role_code, ...input })
  );
  permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
    consolePolicyCatalog([])
  );
  permissionsApi.replaceSettingsConsolePolicyOrder.mockReset();
  rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
    role_code: 'member',
    groups: []
  });
  rolesApi.replaceSettingsRoleConsolePolicy.mockResolvedValue(undefined);
  dataModelsApi.fetchSettingsAllDataModels.mockResolvedValue(
    defaultDataModels()
  );
});

export { permissionsApi, renderPanel, rolesApi };
