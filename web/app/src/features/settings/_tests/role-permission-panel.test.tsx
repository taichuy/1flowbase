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
  fetchSettingsRoles: vi.fn(),
  createSettingsRole: vi.fn(),
  updateSettingsRole: vi.fn(),
  deleteSettingsRole: vi.fn(),
  fetchSettingsRolePermissions: vi.fn(),
  replaceSettingsRolePermissions: vi.fn(),
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

const defaultDataPolicy = {
  role_code: 'manager',
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
      view_scope_override: null,
      update_scope_override: 'own',
      delete_scope_override: 'scope_all'
    },
    {
      data_model_id: 'model-customers',
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
      data_source_instance_id: 'source-1',
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
      data_source_instance_id: 'source-1',
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
        code: 'manager',
        name: 'Manager',
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
      role_code: 'manager',
      permission_codes: []
    });
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
          'manager',
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

    await waitFor(() => {
      expect(
        within(defaultSection).getByRole('switch', { name: '新增' })
      ).toBeChecked();
    });
    fireEvent.click(
      within(defaultSection).getByRole('radio', { name: '查看 本空间' })
    );
    fireEvent.click(
      within(defaultSection).getByRole('radio', { name: '更新 仅自己' })
    );
    fireEvent.click(
      screen.getByRole('button', { name: '保存数据权限' })
    );

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'manager',
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
              view_scope_override: null,
              update_scope_override: 'own',
              delete_scope_override: 'scope_all'
            },
            {
              data_model_id: 'model-customers',
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

    fireEvent.click(
      within(ordersRow).getByRole('radio', { name: '查看 本空间' })
    );
    fireEvent.click(
      within(ordersRow).getByRole('radio', { name: '更新 继承' })
    );
    fireEvent.click(
      within(ordersRow).getByRole('radio', { name: '删除 仅自己' })
    );
    fireEvent.click(screen.getByRole('button', { name: '保存数据权限' }));

    await waitFor(() => {
      expect(rolesApi.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'manager',
        expect.objectContaining({
          model_policies: expect.arrayContaining([
            {
              data_model_id: 'model-orders',
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
      within(defaultSection).getByRole('switch', { name: '查看' })
    ).toBeDisabled();
    expect(
      within(defaultSection).getByRole('radio', { name: '查看 仅自己' })
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: '保存数据权限' })).toBeDisabled();
  }, 20000);
});
