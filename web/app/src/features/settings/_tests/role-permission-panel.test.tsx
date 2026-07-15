import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import fs from 'node:fs';
import path from 'node:path';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  fetchSettingsConsolePolicyCatalog: vi.fn()
}));

const dataModelsApi = vi.hoisted(() => ({
  settingsAllDataModelsQueryKey: ['settings', 'data-models', 'models', 'all'],
  fetchSettingsAllDataModels: vi.fn()
}));

vi.mock('../api/roles', () => rolesApi);
vi.mock('../api/permissions', () => permissionsApi);
vi.mock('../api/data-models', () => dataModelsApi);

import { AppProviders } from '../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { RolePermissionPanel } from '../components/RolePermissionPanel';

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
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['role_permission.manage.all']
    }
  });
}

function setPreferredLocale(locale: 'zh_Hans' | 'en_US') {
  useAuthStore.setState((state) => ({
    me: state.me ? { ...state.me, preferred_locale: locale } : state.me
  }));
}

function renderPanel(canManageRoles = true) {
  return render(
    <AppProviders>
      <RolePermissionPanel canManageRoles={canManageRoles} />
    </AppProviders>
  );
}

async function selectDefaultPolicyScope(
  section: HTMLElement,
  actionLabel: string,
  optionLabel: string
) {
  fireEvent.mouseDown(
    within(section).getByRole('combobox', { name: `${actionLabel} 作用域` })
  );
  const optionMatches = await screen.findAllByTitle(optionLabel);
  fireEvent.click(optionMatches[optionMatches.length - 1]);
}

async function selectPolicyCombobox(
  scope: HTMLElement,
  comboboxName: string,
  optionLabel: string
) {
  fireEvent.mouseDown(within(scope).getByRole('combobox', { name: comboboxName }));
  const optionMatches = await screen.findAllByTitle(optionLabel);
  fireEvent.click(optionMatches[optionMatches.length - 1]);
}

const defaultDataPolicy = {
  role_code: 'member',
  default_policy: {
    can_view: true,
    can_create: true,
    can_update: true,
    can_delete: false,
    default_view_scope: 'own',
    default_update_scope: 'scope_all',
    default_delete_scope: 'own'
  },
  model_policies: [
    {
      data_model_id: 'model-orders',
      can_create_override: null,
      view_scope_override: null,
      update_scope_override: 'own',
      delete_scope_override: 'scope_all'
    },
    {
      data_model_id: 'model-customers',
      can_create_override: null,
      view_scope_override: null,
      update_scope_override: null,
      delete_scope_override: null
    }
  ]
};

const policyModeOptions = [
  { value: 'disabled', label: '不授予', description: '不授予任何操作' },
  { value: 'full', label: '全部操作', description: '授予全部操作' },
  { value: 'custom', label: '按操作配置', description: '仅授予显式操作' }
];

const allRowScopeOptions = [
  { value: 'disabled', label: '关闭', description: '不授予访问' },
  { value: 'own', label: '仅自己', description: '仅自己的记录' },
  { value: 'scope_all', label: '当前空间', description: '当前空间中的记录' }
];

function consolePolicyCatalog(
  groups: unknown[],
  {
    locale = 'zh_Hans',
    groupModeOptions = policyModeOptions
  }: {
    locale?: 'zh_Hans' | 'en_US';
    groupModeOptions?: typeof policyModeOptions;
  } = {}
) {
  return {
    schema_version: '2026-07-15',
    locale,
    group_mode_options: groupModeOptions,
    groups,
    resources: []
  };
}

function defaultDataModels() {
  return [
    {
      id: 'model-orders',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      code: 'orders',
      title: 'Orders',
      status: 'published',
      runtime_availability: 'available',
      data_source_id: 'source-1',
      source_kind: 'main_source',
      external_resource_key: null,
      external_table_id: null,
      physical_table_name: 'orders',
      acl_namespace: 'data_model:orders',
      audit_namespace: 'data_model:orders',
      builtin_kind: null,
      capabilities: {
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
      },
      fields: []
    },
    {
      id: 'model-customers',
      scope_kind: 'workspace',
      scope_id: 'workspace-1',
      code: 'customers',
      title: 'Customers',
      status: 'published',
      runtime_availability: 'available',
      data_source_id: 'source-1',
      source_kind: 'main_source',
      external_resource_key: null,
      external_table_id: null,
      physical_table_name: 'customers',
      acl_namespace: 'data_model:customers',
      audit_namespace: 'data_model:customers',
      builtin_kind: null,
      capabilities: {
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
      },
      fields: []
    }
  ];
}

describe('RolePermissionPanel', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
    authenticate();
    rolesApi.fetchSettingsRoles.mockResolvedValue([
      {
        code: 'member',
        name: 'Member',
        introduction: '默认管理角色',
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
    rolesApi.replaceSettingsRoleDataPolicy.mockResolvedValue(undefined);
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: []
    });
    rolesApi.replaceSettingsRoleConsolePolicy.mockResolvedValue(undefined);
    dataModelsApi.fetchSettingsAllDataModels.mockResolvedValue(
      defaultDataModels()
    );
  });

  test('AC-008 stacks the role rail above policy content at 390px', async () => {
    renderPanel();

    expect(await screen.findByTestId('role-permission-layout')).toHaveClass(
      'role-permission-layout'
    );
    expect(screen.getByTestId('role-permission-rail')).toHaveClass(
      'role-permission-layout__rail'
    );
    expect(screen.getByTestId('role-permission-content')).toHaveClass(
      'role-permission-layout__content'
    );

    const layoutCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../components/role-permissions/role-permission-panel.css'
      ),
      'utf8'
    );
    const mobileRule = layoutCss.match(
      /@media \(max-width: 767px\) \{([\s\S]*?)\n\}/
    )?.[1];

    expect(mobileRule).toContain(
      '.role-permission-layout {\n    flex-direction: column;'
    );
    expect(mobileRule).toContain(
      '.role-permission-layout__rail {\n    width: 100%;'
    );
    expect(mobileRule).toContain(
      '.role-permission-layout__content {\n    min-width: 0;'
    );
  });

  test('AC-003/008/009 renders backend-owned Other labels without exposing codes', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
      schema_version: '2026-07-14',
      locale: 'zh_Hans',
      group_mode_options: policyModeOptions,
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          label: '应用管理',
          description: '管理当前空间中的应用',
          operations: [
            {
              operation_id: 'applications.read',
              label: '查询应用',
              description: '查看应用记录',
              order: 1,
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'application',
                action_code: 'read'
              }
            },
            {
              operation_id: 'applications.publish',
              label: '发布应用',
              description: null,
              order: 2,
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        },
        {
          kind: 'other',
          group_id: 'other.general',
          label: '其他',
          description: '其他已注册操作',
          operations: [
            {
              operation_id: 'audit.export',
              label: '导出审计记录',
              description: null,
              order: 1,
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ],
      resources: [
        {
          resource_code: 'application',
          label: '应用',
          description: null,
          actions: [
            { action_code: 'read', label: '查询', description: null }
          ]
        }
      ]
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          mode: 'full',
          operations: [
            {
              operation_id: 'applications.read',
              kind: 'row',
              scope: 'scope_all'
            },
            {
              operation_id: 'applications.publish',
              kind: 'simple',
              enabled: true
            }
          ]
        },
        {
          kind: 'other',
          group_id: 'other.general',
          mode: 'disabled',
          operations: [
            {
              operation_id: 'audit.export',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    const { container } = renderPanel();

    const backendTab = await screen.findByRole('tab', { name: '后台系统设置' });
    expect(backendTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '动态路由',
      '表-通用配置',
      '表-单独配置',
      '后台系统设置',
      '其他'
    ]);

    expect(
      screen.getByRole('columnheader', { name: '后台设置' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '开放权限' })
    ).toBeInTheDocument();
    expect(screen.getByText('应用管理')).toBeInTheDocument();
    expect(screen.getByText('全部操作')).toBeInTheDocument();
    expect(screen.queryByText('applications.read')).not.toBeInTheDocument();
    expect(screen.queryByText('application')).not.toBeInTheDocument();
    expect(screen.queryByText(/\{.*operation_id/)).not.toBeInTheDocument();
    expect(container.innerHTML).not.toContain('settings.applications');
    expect(container.innerHTML).not.toContain('applications.read');
    expect(container.innerHTML).not.toContain('application');

    fireEvent.click(screen.getByRole('tab', { name: '其他' }));
    expect(screen.getByText('不授予')).toBeInTheDocument();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: '详细配置 其他' })
    );
    const otherDrawer = await screen.findByRole('dialog');
    expect(within(otherDrawer).getByText('导出审计记录')).toBeInTheDocument();

    expect(screen.queryByRole('tab', { name: '基础通用' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: '系统管理' })).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: 'Agent 应用' })).not.toBeInTheDocument();
  });

  test('AC-003 selects Other when the catalog has no SettingsFeature group', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'other',
          group_id: 'other.general',
          label: '其他',
          description: '其他已注册操作',
          operations: []
        }
      ])
    );

    renderPanel();

    expect(await screen.findByRole('tab', { name: '其他' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(screen.getByText('其他已注册操作')).toBeInTheDocument();
  });

  test('AC-004 toggles group mode and saves simple/resource scopes from the detail drawer', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue({
      schema_version: '2026-07-14',
      locale: 'zh_Hans',
      group_mode_options: policyModeOptions,
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          label: '应用管理',
          description: null,
          operations: [
            {
              operation_id: 'applications.read',
              label: '查询应用',
              description: null,
              order: 1,
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'application',
                action_code: 'read'
              }
            },
            {
              operation_id: 'applications.publish',
              label: '发布应用',
              description: null,
              order: 2,
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ],
      resources: [
        {
          resource_code: 'application',
          label: '应用',
          description: null,
          actions: [{ action_code: 'read', label: '查询', description: null }]
        }
      ]
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.applications',
          mode: 'custom',
          operations: [
            {
              operation_id: 'applications.read',
              kind: 'row',
              scope: 'own'
            },
            {
              operation_id: 'applications.publish',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    renderPanel();

    const accessCheckbox = await screen.findByRole('checkbox', {
      name: '开放 应用管理 权限'
    });
    expect(accessCheckbox).toBeChecked();
    expect(screen.getByText('按操作配置')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '详细配置 应用管理' }));
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('查询应用')).toBeInTheDocument();
    expect(within(drawer).getByRole('checkbox', { name: '发布应用' })).not.toBeChecked();

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: '查询应用 作用域' })
    );
    fireEvent.click((await screen.findAllByTitle('当前空间')).at(-1)!);
    fireEvent.click(within(drawer).getByRole('checkbox', { name: '发布应用' }));
    fireEvent.click(within(drawer).getByRole('button', { name: '保存权限配置' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.applications',
              mode: 'custom',
              operations: [
                {
                  operation_id: 'applications.read',
                  kind: 'row',
                  scope: 'scope_all'
                },
                {
                  operation_id: 'applications.publish',
                  kind: 'simple',
                  enabled: true
                }
              ]
            }
          ]
        },
        'csrf-123'
      );
    });

    fireEvent.click(accessCheckbox);
    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            expect.objectContaining({
              group_id: 'settings.applications',
              mode: 'disabled'
            })
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-004 renders a catalog group absent from stored policy as disabled and enables it as full', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.missing',
          label: '缺失策略组',
          description: '后端 catalog 中存在，角色策略中缺失',
          operations: [
            {
              operation_id: 'missing.read',
              label: '查看缺失资源',
              description: '读取当前空间资源',
              order: 1,
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'missing_resource',
                action_code: 'read'
              }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: []
    });

    renderPanel();

    const accessCheckbox = await screen.findByRole('checkbox', {
      name: '开放 缺失策略组 权限'
    });
    expect(accessCheckbox).not.toBeChecked();
    expect(screen.getByText('不授予')).toBeInTheDocument();

    fireEvent.click(accessCheckbox);

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.missing',
              mode: 'full',
              operations: []
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-004 projects the server full profile before converting a detail edit to explicit custom policies', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.full-profile',
          label: '完整策略组',
          description: '',
          operations: [
            {
              operation_id: 'full-profile.read',
              label: '查询记录',
              description: '',
              order: 1,
              full_profile: { kind: 'row', scope: 'scope_all' },
              allowed_row_scopes: allRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'full_profile_resource',
                action_code: 'read'
              }
            },
            {
              operation_id: 'full-profile.publish',
              label: '发布记录',
              description: '',
              order: 2,
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.full-profile',
          mode: 'full',
          operations: []
        }
      ]
    });

    renderPanel();

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '详细配置 完整策略组' },
        { timeout: 5000 }
      )
    );
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('当前空间')).toBeInTheDocument();
    expect(
      within(drawer).getByRole('checkbox', { name: '发布记录' })
    ).toBeChecked();

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: '查询记录 作用域' })
    );
    fireEvent.click((await screen.findAllByTitle('仅自己')).at(-1)!);
    fireEvent.click(within(drawer).getByRole('button', { name: '保存权限配置' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.full-profile',
              mode: 'custom',
              operations: [
                {
                  operation_id: 'full-profile.read',
                  kind: 'row',
                  scope: 'own'
                },
                {
                  operation_id: 'full-profile.publish',
                  kind: 'simple',
                  enabled: true
                }
              ]
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-004 restores a custom group to full without retaining explicit operations', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'settings_feature',
          group_id: 'settings.restore-full',
          label: '可恢复策略组',
          description: '',
          operations: [
            {
              operation_id: 'restore-full.publish',
              label: '发布',
              description: '',
              order: 1,
              full_profile: { kind: 'simple', enabled: true },
              allowed_row_scopes: [],
              authorization: { kind: 'simple' }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'settings_feature',
          group_id: 'settings.restore-full',
          mode: 'custom',
          operations: [
            {
              operation_id: 'restore-full.publish',
              kind: 'simple',
              enabled: false
            }
          ]
        }
      ]
    });

    renderPanel();

    fireEvent.click(
      await screen.findByRole(
        'button',
        { name: '恢复 全部操作' },
        { timeout: 5000 }
      )
    );

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleConsolePolicy).toHaveBeenCalledWith(
        'member',
        {
          groups: [
            {
              kind: 'settings_feature',
              group_id: 'settings.restore-full',
              mode: 'full',
              operations: []
            }
          ]
        },
        'csrf-123'
      );
    });
  });

  test('AC-004/009 renders only a row operation\'s backend-supplied allowed scopes', async () => {
    const limitedRowScopeOptions = [
      { value: 'disabled', label: '目录关闭', description: '不授予访问' },
      { value: 'own', label: '目录中的本人记录', description: '仅自己的记录' }
    ];
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockResolvedValue(
      consolePolicyCatalog([
        {
          kind: 'other',
          group_id: 'other.limited-scopes',
          label: '受限范围',
          description: '',
          operations: [
            {
              operation_id: 'limited-scope.read',
              label: '读取受限记录',
              description: '',
              order: 1,
              full_profile: { kind: 'row', scope: 'own' },
              allowed_row_scopes: limitedRowScopeOptions,
              authorization: {
                kind: 'resource_action',
                resource_code: 'limited_scope_resource',
                action_code: 'read'
              }
            }
          ]
        }
      ])
    );
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'other',
          group_id: 'other.limited-scopes',
          mode: 'custom',
          operations: [
            {
              operation_id: 'limited-scope.read',
              kind: 'row',
              scope: 'own'
            }
          ]
        }
      ]
    });

    const { container } = renderPanel();
    fireEvent.click(await screen.findByRole('tab', { name: '其他' }));
    fireEvent.click(
      screen.getByRole('button', { name: '详细配置 受限范围' })
    );
    const drawer = await screen.findByRole('dialog');

    fireEvent.mouseDown(
      within(drawer).getByRole('combobox', { name: '读取受限记录 作用域' })
    );
    expect((await screen.findAllByTitle('目录关闭')).length).toBeGreaterThan(0);
    expect(
      (await screen.findAllByTitle('目录中的本人记录')).length
    ).toBeGreaterThan(0);
    expect(screen.queryByTitle('当前空间')).not.toBeInTheDocument();
    expect(screen.queryByTitle('本空间')).not.toBeInTheDocument();
    expect(container.innerHTML).not.toContain('other.limited-scopes');
    expect(container.innerHTML).not.toContain('limited-scope.read');
    expect(container.innerHTML).not.toContain('limited_scope_resource');
  });

  test('dynamic route group selects descendants without persisting the group id', async () => {
    renderPanel();

    fireEvent.click(await screen.findByRole('tab', { name: '动态路由' }));

    expect(await screen.findByText('工作台')).toBeInTheDocument();
    expect(screen.queryByText(/pgr3083h/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select 工作台' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleFrontstageRoutes).toHaveBeenCalledWith(
        'member',
        {
          page_ids: ['child-page'],
          tab_ids: ['child-tab']
        },
        'csrf-123'
      );
    });
  });

  test(
    'submits auto_grant_new_permissions and is_default_member_role from the create and edit dialogs',
    async () => {
      renderPanel();

      await screen.findByRole('button', { name: /新建角色/ });

      fireEvent.click(screen.getByRole('button', { name: /新建角色/ }));

      const createDialog = await screen.findByRole('dialog');
      fireEvent.change(within(createDialog).getByLabelText('角色名称'), {
        target: { value: 'QA' }
      });
      fireEvent.change(within(createDialog).getByLabelText('角色编码'), {
        target: { value: 'qa' }
      });
      fireEvent.click(
        within(createDialog).getByRole('checkbox', { name: '自动接收后续新增权限' })
      );
      fireEvent.click(within(createDialog).getByRole('button', { name: /确\s*定/u }));

      await waitFor(() => {
        expect(rolesApi.createSettingsRole).toHaveBeenCalledWith(
          {
            code: 'qa',
            name: 'QA',
            introduction: '',
            auto_grant_new_permissions: true,
            is_default_member_role: false
          },
          'csrf-123'
        );
      });

      fireEvent.click(screen.getByRole('button', { name: /编辑基本信息/ }));

      const editDialog = await screen.findByRole('dialog');
      expect(
        within(editDialog).getByRole('checkbox', { name: '默认新用户角色' })
      ).toBeChecked();
      expect(
        within(editDialog).getByRole('checkbox', { name: '自动接收后续新增权限' })
      ).not.toBeChecked();

      fireEvent.change(within(editDialog).getByLabelText('角色名称'), {
        target: { value: 'Manager Updated' }
      });
      fireEvent.click(
        within(editDialog).getByRole('checkbox', { name: '自动接收后续新增权限' })
      );
      fireEvent.click(
        within(editDialog).getByRole('checkbox', { name: '默认新用户角色' })
      );
      fireEvent.click(within(editDialog).getByRole('button', { name: /确\s*定/u }));

      await waitFor(() => {
        expect(rolesApi.updateSettingsRole).toHaveBeenCalledWith(
          'member',
          {
            name: 'Manager Updated',
            introduction: '默认管理角色',
            auto_grant_new_permissions: true,
            is_default_member_role: false
          },
          'csrf-123'
        );
      });
    },
    20000
  );

  test('submits default data policy without a create scope', async () => {
    renderPanel();

    await screen.findByRole('tab', { name: '动态路由' });
    expect(
      screen.getAllByRole('tab').map((tab) => tab.textContent)
    ).toEqual([
      '动态路由',
      '表-通用配置',
      '表-单独配置'
    ]);
    expect(screen.getAllByRole('tablist')).toHaveLength(1);
    expect(
      screen.queryByText('Data Model 数据权限')
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: '表-通用配置' }));

    expect(
      screen.queryByText('Data Model 数据权限')
    ).not.toBeInTheDocument();

    const defaultSection = await screen.findByRole('region', {
      name: '默认策略'
    });
    expect(
      within(defaultSection).queryByRole('radiogroup', { name: '新增范围' })
    ).not.toBeInTheDocument();
    expect(
      within(defaultSection).queryByRole('switch', { name: '新增' })
    ).not.toBeInTheDocument();
    expect(within(defaultSection).queryByRole('tree')).not.toBeInTheDocument();
    expect(within(defaultSection).getByRole('table')).toBeInTheDocument();
    expect(
      within(defaultSection).getByRole('checkbox', { name: '新增 启用' })
    ).toBeInTheDocument();
    expect(
      within(defaultSection).getByRole('combobox', { name: '查看 作用域' })
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        within(defaultSection).getByRole('checkbox', {
          name: '新增 启用'
        })
      ).toBeChecked();
    });
    await selectDefaultPolicyScope(defaultSection, '查看', '本空间');
    await selectDefaultPolicyScope(defaultSection, '更新', '仅自己');
    fireEvent.click(
      screen.getByRole('button', { name: '保存数据权限' })
    );

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'member',
        {
          default_policy: {
            can_view: true,
            can_create: true,
            can_update: true,
            can_delete: false,
            default_view_scope: 'scope_all',
            default_update_scope: 'own',
            default_delete_scope: 'own'
          },
          model_policies: [
            {
              data_model_id: 'model-orders',
              can_create_override: null,
              view_scope_override: null,
              update_scope_override: 'own',
              delete_scope_override: 'scope_all'
            },
            {
              data_model_id: 'model-customers',
              can_create_override: null,
              view_scope_override: null,
              update_scope_override: null,
              delete_scope_override: null
            }
          ]
        },
        'csrf-123'
      );
    });
  }, 20000);

  test('submits per-model override payload and hides system_all choices', async () => {
    renderPanel();

    await screen.findByRole('tab', { name: '动态路由' });
    fireEvent.click(screen.getByRole('tab', { name: '表-单独配置' }));

    const ordersRow = await screen.findByRole('row', { name: /Orders orders/ });
    expect(screen.queryByText('所有数据')).not.toBeInTheDocument();
    expect(
      within(ordersRow).queryByRole('radio', { name: '查看 本空间' })
    ).not.toBeInTheDocument();
    expect(
      within(ordersRow).getByRole('combobox', { name: '查看 Orders' })
    ).toBeInTheDocument();
    expect(
      within(ordersRow).getByRole('checkbox', { name: '新增 Orders' })
    ).toBeChecked();

    fireEvent.click(within(ordersRow).getByRole('checkbox', { name: '新增 Orders' }));
    await selectPolicyCombobox(ordersRow, '查看 Orders', '本空间');
    await selectPolicyCombobox(ordersRow, '更新 Orders', '继承');
    await selectPolicyCombobox(ordersRow, '删除 Orders', '仅自己');
    fireEvent.click(screen.getByRole('button', { name: '保存数据权限' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'member',
        expect.objectContaining({
          model_policies: expect.arrayContaining([
            {
              data_model_id: 'model-orders',
              can_create_override: false,
              view_scope_override: 'scope_all',
              update_scope_override: null,
              delete_scope_override: 'own'
            }
          ])
        }),
        'csrf-123'
      );
    });
  }, 20000);

  test('disables data policy controls when the user cannot manage roles', async () => {
    renderPanel(false);

    await screen.findByRole('tab', { name: '动态路由' });
    fireEvent.click(screen.getByRole('tab', { name: '表-通用配置' }));

    const defaultSection = await screen.findByRole('region', {
      name: '默认策略'
    });

    expect(
      within(defaultSection).getByRole('checkbox', { name: '查看 启用' })
    ).toBeDisabled();
    expect(
      within(defaultSection).getByRole('combobox', { name: '查看 作用域' })
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: '保存数据权限' })).toBeDisabled();
  }, 20000);

  test('AC-009 requests the active locale and renders backend labels without exposing policy codes', async () => {
    permissionsApi.fetchSettingsConsolePolicyCatalog.mockImplementation(async (locale) => {
      const isEnglish = locale === 'en_US';
      return {
        schema_version: '2026-07-14',
        locale,
        group_mode_options: policyModeOptions,
        groups: [
          {
            kind: 'other' as const,
            group_id: 'other.general',
            label: isEnglish ? 'Other settings' : '其他设置',
            description: null,
            operations: [
              {
                operation_id: 'audit.export',
                label: isEnglish ? 'Export audit records' : '导出审计记录',
                description: null,
                order: 1,
                full_profile: { kind: 'simple', enabled: true },
                allowed_row_scopes: [],
                authorization: { kind: 'simple' as const }
              }
            ]
          }
        ],
        resources: []
      };
    });
    rolesApi.fetchSettingsRoleConsolePolicy.mockResolvedValue({
      role_code: 'member',
      groups: [
        {
          kind: 'other',
          group_id: 'other.general',
          mode: 'disabled',
          operations: [
            { operation_id: 'audit.export', kind: 'simple', enabled: false }
          ]
        }
      ]
    });

    setPreferredLocale('en_US');
    await appI18n.changeLanguage('en_US');
    const view = renderPanel();
    await waitFor(() => {
      expect(permissionsApi.fetchSettingsConsolePolicyCatalog).toHaveBeenCalledWith(
        'en_US'
      );
    });
    fireEvent.click(await screen.findByRole('tab', { name: 'Others' }));
    expect(await screen.findByText('Other settings')).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: 'Configure Other settings' })
    );
    const englishDrawer = await screen.findByRole('dialog');
    expect(
      within(englishDrawer).getByText('Export audit records')
    ).toBeInTheDocument();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    expect(screen.queryByText('audit.export')).not.toBeInTheDocument();
    expect(permissionsApi.settingsConsolePolicyCatalogQueryKey).toHaveBeenCalledWith(
      'en_US'
    );
    view.unmount();

    setPreferredLocale('zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    renderPanel();
    await waitFor(() => {
      expect(permissionsApi.fetchSettingsConsolePolicyCatalog).toHaveBeenCalledWith(
        'zh_Hans'
      );
    });
    fireEvent.click(await screen.findByRole('tab', { name: '其他' }));
    expect(await screen.findByText('其他设置')).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: '详细配置 其他设置' })
    );
    const chineseDrawer = await screen.findByRole('dialog');
    expect(within(chineseDrawer).getByText('导出审计记录')).toBeInTheDocument();
    expect(screen.queryByText('other.general')).not.toBeInTheDocument();
    expect(screen.queryByText('audit.export')).not.toBeInTheDocument();
    expect(permissionsApi.settingsConsolePolicyCatalogQueryKey).toHaveBeenCalledWith(
      'zh_Hans'
    );
  }, 20000);

});
