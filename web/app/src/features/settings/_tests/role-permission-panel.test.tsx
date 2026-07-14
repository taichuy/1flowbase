import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const rolesApi = vi.hoisted(() => ({
  settingsRolesQueryKey: ['settings', 'roles'],
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
  fetchSettingsRolePermissions: vi.fn(),
  replaceSettingsRolePermissions: vi.fn(),
  fetchSettingsRoleFrontstageRoutes: vi.fn(),
  replaceSettingsRoleFrontstageRoutes: vi.fn(),
  fetchSettingsRoleDataPolicy: vi.fn(),
  replaceSettingsRoleDataPolicy: vi.fn()
}));

const permissionsApi = vi.hoisted(() => ({
  settingsPermissionsQueryKey: ['settings', 'permissions'],
  fetchSettingsPermissions: vi.fn()
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
  beforeEach(() => {
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
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([]);
    dataModelsApi.fetchSettingsAllDataModels.mockResolvedValue(
      defaultDataModels()
    );
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

    await screen.findByRole('tab', { name: '基础通用' });
    expect(
      screen.getAllByRole('tab').map((tab) => tab.textContent)
    ).toEqual([
      '基础通用',
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

    await screen.findByRole('tab', { name: '基础通用' });
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

    await screen.findByRole('tab', { name: '基础通用' });
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

  test('groups unknown permission resources under 其他 with the raw resource key', async () => {
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([
      {
        code: 'user.view.all',
        resource: 'user',
        action: 'view',
        scope: 'all',
        name: '查看用户'
      },
      {
        code: 'custom_resource.audit.all',
        resource: 'custom_resource',
        action: 'audit',
        scope: 'all',
        name: 'Audit custom resource'
      }
    ]);
    rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
      role_code: 'member',
      permission_codes: ['custom_resource.audit.all']
    });

    renderPanel();

    fireEvent.click(await screen.findByRole('tab', { name: '其他' }));

    const resourceNode = screen
      .getByText('custom_resource')
      .closest('.ant-tree-treenode');
    const switcher = resourceNode?.querySelector<HTMLElement>('.ant-tree-switcher');
    expect(switcher).toBeTruthy();
    fireEvent.click(switcher!);

    expect(await screen.findByText('Audit custom resource')).toBeInTheDocument();
  }, 20000);

  test('groups settings route visibility permissions under 路由页面 with a friendly resource label', async () => {
    permissionsApi.fetchSettingsPermissions.mockResolvedValue([
      {
        code: 'settings_route.visible.settings.docs',
        resource: 'settings_route',
        action: 'visible',
        scope: 'settings.docs',
        name: '可见 API 文档'
      },
      {
        code: 'settings_route.visible.settings.roles',
        resource: 'settings_route',
        action: 'visible',
        scope: 'settings.roles',
        name: '可见权限管理'
      }
    ]);
    rolesApi.fetchSettingsRolePermissions.mockResolvedValue({
      role_code: 'member',
      permission_codes: ['settings_route.visible.settings.roles']
    });

    renderPanel();

    fireEvent.click(await screen.findByRole('tab', { name: '路由页面' }));

    expect(screen.getByText('设置页面')).toBeInTheDocument();
    expect(screen.queryByText('settings_route')).not.toBeInTheDocument();
  }, 20000);
});
