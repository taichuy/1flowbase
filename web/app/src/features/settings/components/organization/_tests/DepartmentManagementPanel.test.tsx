import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
const api = vi.hoisted(() => ({
  fetchSettingsDepartments: vi.fn(),
  createSettingsDepartment: vi.fn(),
  updateSettingsDepartment: vi.fn(),
  deleteSettingsDepartment: vi.fn(),
  fetchSettingsMemberRoleOptions: vi.fn(),
  settingsDepartmentsQueryKey: ['settings', 'departments'],
  settingsMemberRoleOptionsQueryKey: ['settings', 'members', 'role-options']
}));
vi.mock('../../../api/departments', () => api);
import { AppProviders } from '../../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import {
  DepartmentManagementPanel,
  type DepartmentAccess
} from '../DepartmentManagementPanel';
const denied: DepartmentAccess = {
  can_list: false,
  can_create: false,
  can_update: false,
  can_delete: false,
  can_assign_roles: false,
  can_replace_member_departments: false
};
describe('department operations and access', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    useAuthStore.setState({ csrfToken: 'csrf' });
    api.fetchSettingsDepartments.mockResolvedValue([
      {
        id: 'a',
        name: 'Engineering',
        parent_id: null,
        role_codes: ['operator'],
        member_count: 9
      }
    ]);
    api.createSettingsDepartment.mockResolvedValue({
      id: 'b',
      name: 'Support',
      parent_id: null,
      role_codes: [],
      member_count: 0
    });
  });
  test('does not request the department list when access is denied', () => {
    render(
      <AppProviders>
        <DepartmentManagementPanel access={denied} />
      </AppProviders>
    );
    expect(api.fetchSettingsDepartments).not.toHaveBeenCalled();
    expect(screen.getByText('暂无组织查看权限')).toBeInTheDocument();
  });
  test('list-only access displays backend counts and hides every granting action', async () => {
    render(
      <AppProviders>
        <DepartmentManagementPanel access={{ ...denied, can_list: true }} />
      </AppProviders>
    );
    expect(await screen.findByText('Engineering')).toBeInTheDocument();
    expect(screen.getByText('9')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /新增根部门|新增子部门|编辑|删除/ })
    ).not.toBeInTheDocument();
    expect(api.fetchSettingsMemberRoleOptions).not.toHaveBeenCalled();
  });
  test('creation without role assignment access sends an empty role set', async () => {
    render(
      <AppProviders>
        <DepartmentManagementPanel
          access={{ ...denied, can_list: true, can_create: true }}
        />
      </AppProviders>
    );
    fireEvent.click(screen.getByRole('button', { name: /新增根部门/ }));
    expect(
      screen.queryByRole('combobox', { name: '部门角色' })
    ).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: 'Support' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(api.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'Support', parent_id: null, role_codes: [] },
        'csrf'
      )
    );
  });
});
