import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  fetchSettingsRoles: vi.fn()
}));

const navigateMock = vi.hoisted(() => vi.fn());
const MEMBER_EDIT_PROFILE_TEST_TIMEOUT = 30_000;

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => navigateMock
}));
vi.mock('../api/members', () => membersApi);
vi.mock('../api/roles', () => rolesApi);
const departmentsApi = vi.hoisted(() => ({
  settingsDepartmentsQueryKey: ['settings', 'departments'],
  settingsMemberRoleOptionsQueryKey: ['settings', 'members', 'role-options'],
  fetchSettingsDepartments: vi.fn(),
  createSettingsDepartment: vi.fn(),
  updateSettingsDepartment: vi.fn(),
  replaceSettingsMemberDepartments: vi.fn()
}));
vi.mock('../api/departments', () => ({
  ...departmentsApi,
  fetchSettingsMemberRoleOptions: rolesApi.fetchSettingsRoles
}));

import { AppProviders } from '../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { MemberManagementPanel } from '../components/MemberManagementPanel';

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
      nickname: 'Root Nick',
      name: 'Root Name',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['user.view.all', 'user.manage.all']
    }
  });
}

function renderPanel(withDepartments = false, canCreateDepartments = false) {
  return render(
    <AppProviders>
      <MemberManagementPanel
        canManageMembers
        canManageRoleBindings
        canViewDepartments={withDepartments}
        canCreateDepartments={canCreateDepartments}
        canManageMemberDepartments={withDepartments}
      />
    </AppProviders>
  );
}

function findMemberRow(name: RegExp) {
  return screen.findByRole('row', { name });
}

// rc-component shares test-id across ARIA heading associations in tests.
// Locate the real dialog through its visible title, retaining save assertions.
async function findProfileDialog(name: string) {
  const title = await screen.findByText(`编辑用户资料 ${name}`);
  const dialog = title.closest<HTMLElement>('[role="dialog"]');
  if (!dialog) throw new Error('Profile title is not inside a dialog');
  await waitFor(() => expect(dialog).toBeVisible());
  expect(dialog).toHaveAttribute('aria-modal', 'true');
  return dialog;
}

function ignoreCircularReferenceWarning() {
  const originalError = console.error;
  const errorSpy = vi.spyOn(console, 'error').mockImplementation((...args) => {
    if (
      args.some(
        (arg) =>
          typeof arg === 'string' &&
          arg.includes('There may be circular references')
      )
    ) {
      return;
    }

    originalError(...args);
  });

  return () => errorSpy.mockRestore();
}

describe('MemberManagementPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    departmentsApi.fetchSettingsDepartments.mockResolvedValue([
      {
        id: 'department-1',
        name: 'Engineering',
        parent_id: null,
        role_codes: ['operator'],
        member_count: 8
      }
    ]);
    departmentsApi.replaceSettingsMemberDepartments.mockResolvedValue(
      undefined
    );
    membersApi.fetchSettingsMembers.mockResolvedValue([
      {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        name: 'Root Name',
        nickname: 'Root Nick',
        introduction: '',
        default_display_role: 'root',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        department_ids: [],
        primary_department_id: null,
        role_codes: ['root', 'member']
      },
      {
        id: 'user-2',
        account: 'user',
        email: 'user@example.com',
        phone: null,
        name: 'User Name',
        nickname: 'User Nick',
        introduction: '',
        default_display_role: 'member',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        department_ids: [],
        primary_department_id: null,
        role_codes: ['member']
      },
      {
        id: 'user-3',
        account: 'disabled-user',
        email: 'disabled-user@example.com',
        phone: null,
        name: 'Disabled User',
        nickname: 'Disabled Nick',
        introduction: '',
        default_display_role: 'member',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'disabled',
        department_ids: [],
        primary_department_id: null,
        role_codes: ['member']
      }
    ]);
    membersApi.updateSettingsMember.mockResolvedValue({
      id: 'user-1',
      account: 'root',
      email: 'root-next@example.com',
      phone: null,
      name: 'Root Next',
      nickname: 'Root Nick',
      introduction: '',
      default_display_role: 'root',
      email_login_enabled: true,
      phone_login_enabled: false,
      status: 'active',
      role_codes: ['root', 'member', 'operator']
    });
    membersApi.replaceSettingsMemberRoles.mockResolvedValue(undefined);
    membersApi.enableSettingsMember.mockResolvedValue(undefined);
    membersApi.deleteSettingsMember.mockResolvedValue(undefined);
    rolesApi.fetchSettingsRoles.mockResolvedValue([
      {
        code: 'member',
        name: 'Member',
        introduction: '',
        scope_kind: 'workspace',
        is_builtin: true,
        is_editable: true,
        auto_grant_new_permissions: false,
        is_default_member_role: true,
        permission_codes: []
      },
      {
        code: 'operator',
        name: 'Operator',
        introduction: '',
        scope_kind: 'workspace',
        is_builtin: false,
        is_editable: true,
        auto_grant_new_permissions: false,
        is_default_member_role: false,
        permission_codes: []
      }
    ]);
  });

  test('splits identity into avatar, account, name, and nickname columns', async () => {
    renderPanel();

    await waitFor(() => {
      expect(membersApi.fetchSettingsMembers).toHaveBeenCalled();
    });

    expect(
      screen.getByRole('columnheader', { name: '头像' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '账号' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '姓名' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '昵称' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '直接角色' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '用户' })
    ).not.toBeInTheDocument();

    expect(await screen.findByText('root')).toBeInTheDocument();
    expect(screen.getByText('Root Name')).toBeInTheDocument();
    expect(screen.getByText('Root Nick')).toBeInTheDocument();
  });

  test('renames profile action to edit and deletes non-root members after confirmation', async () => {
    renderPanel();

    const rootRow = await findMemberRow(/root.*Root Name.*Root Nick/u);
    const userRow = await findMemberRow(/user.*User Name.*User Nick/u);

    expect(
      within(rootRow).getByRole('button', { name: /编辑$/ })
    ).toBeInTheDocument();
    expect(
      within(rootRow).queryByRole('button', { name: /编辑资料/ })
    ).not.toBeInTheDocument();
    expect(
      within(rootRow).getByRole('button', { name: /删除$/ })
    ).toBeDisabled();

    fireEvent.click(within(userRow).getByRole('button', { name: /删除$/ }));
    const confirm = await screen.findByRole('button', { name: /确认删除/ });
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(membersApi.deleteSettingsMember).toHaveBeenCalledWith(
        'user-2',
        'csrf-123'
      );
    });
  });

  test('restores disabled members after confirmation', async () => {
    renderPanel();

    const disabledRow = await findMemberRow(
      /disabled-user.*Disabled User.*Disabled Nick/u
    );

    expect(
      within(disabledRow).getByRole('button', { name: /恢复$/ })
    ).toBeInTheDocument();

    fireEvent.click(within(disabledRow).getByRole('button', { name: /恢复$/ }));
    const confirm = await screen.findByRole('button', { name: /确认恢复/ });
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(membersApi.enableSettingsMember).toHaveBeenCalledWith(
        'user-3',
        'csrf-123'
      );
    });
  });

  test(
    'saves profile fields and role bindings from the edit profile dialog',
    async () => {
      const restoreCircularReferenceWarning = ignoreCircularReferenceWarning();
      renderPanel();

      try {
        const row = await findMemberRow(/root.*Root Name.*Root Nick/u);
        fireEvent.click(within(row).getByRole('button', { name: /编辑$/ }));

        const dialog = await findProfileDialog('Root Name');
        expect(
          within(dialog).getByRole('combobox', { name: '直接角色' })
        ).toBeInTheDocument();

        fireEvent.change(within(dialog).getByLabelText('姓名'), {
          target: { value: 'Root Next' }
        });
        fireEvent.mouseDown(
          within(dialog).getByRole('combobox', { name: '直接角色' })
        );
        const [operatorOption] = await screen.findAllByText((_, element) => {
          if (!element) {
            return false;
          }

          return (
            element.matches('.ant-select-item-option-content') &&
            element.textContent === 'Operator'
          );
        });
        fireEvent.click(operatorOption);
        fireEvent.click(
          within(dialog).getByRole('button', { name: /保\s*存/ })
        );

        await waitFor(() => {
          expect(membersApi.updateSettingsMember).toHaveBeenCalledWith(
            'user-1',
            {
              name: 'Root Next',
              nickname: 'Root Nick',
              email: 'root@example.com',
              phone: null,
              introduction: ''
            },
            'csrf-123'
          );
        });
        await waitFor(() => {
          expect(membersApi.replaceSettingsMemberRoles).toHaveBeenCalledWith(
            'user-1',
            { role_codes: ['root', 'member', 'operator'] },
            'csrf-123'
          );
        });
      } finally {
        restoreCircularReferenceWarning();
      }
    },
    MEMBER_EDIT_PROFILE_TEST_TIMEOUT
  );
  test('selecting an organization requests the server subtree and returning to all removes the filter', async () => {
    renderPanel(true);
    fireEvent.click(await screen.findByText('Engineering'));
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers).toHaveBeenCalledWith(
        'department-1'
      )
    );
    fireEvent.click(within(screen.getByRole('tree')).getByText('组织'));
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers).toHaveBeenLastCalledWith(
        undefined
      )
    );
    expect(screen.getByText('8')).toBeInTheDocument();
  });
  test('root browsing includes unassigned users and survives an empty organization or search', async () => {
    departmentsApi.fetchSettingsDepartments.mockResolvedValue([]);
    renderPanel(true);
    expect(await screen.findByText('User Name')).toBeInTheDocument();
    expect(screen.getByRole('tree')).toBeInTheDocument();
    expect(
      within(screen.getByRole('tree')).getByText('组织')
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /全部用户/ })
    ).not.toBeInTheDocument();
    expect(screen.getAllByRole('heading', { name: '组织' })).toHaveLength(2);
    fireEvent.change(screen.getByRole('textbox', { name: '搜索组织' }), {
      target: { value: 'missing' }
    });
    expect(
      within(screen.getByRole('tree')).getByText('组织')
    ).toBeInTheDocument();
    expect(membersApi.fetchSettingsMembers).toHaveBeenLastCalledWith(undefined);
    expect(
      screen.queryByRole('button', { name: /新增部门/ })
    ).not.toBeInTheDocument();
  });
  test('nested departments select the API subtree without filtering or duplicating its response', async () => {
    departmentsApi.fetchSettingsDepartments.mockResolvedValue([
      {
        id: 'department-1',
        name: 'Engineering',
        parent_id: null,
        role_codes: [],
        member_count: 2
      },
      {
        id: 'department-2',
        name: 'Development',
        parent_id: 'department-1',
        role_codes: [],
        member_count: 1
      }
    ]);
    renderPanel(true);
    fireEvent.click(await screen.findByText('Development'));
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers).toHaveBeenLastCalledWith(
        'department-2'
      )
    );
    expect(
      screen.getByRole('heading', { name: 'Development' })
    ).toBeInTheDocument();
    fireEvent.click(within(screen.getByRole('tree')).getByText('Engineering'));
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers).toHaveBeenLastCalledWith(
        'department-1'
      )
    );
    expect(await screen.findByText('User Name')).toBeInTheDocument();
    expect(screen.getAllByText('User Name')).toHaveLength(1);
  });
  test('creates a root department with no parent and refreshes the organization tree and members', async () => {
    departmentsApi.createSettingsDepartment.mockImplementation(async () => {
      departmentsApi.fetchSettingsDepartments.mockResolvedValue([
        {
          id: 'new-department',
          name: 'Support',
          parent_id: null,
          role_codes: [],
          member_count: 4
        }
      ]);
      return { id: 'new-department' };
    });
    renderPanel(true, true);
    fireEvent.click(await screen.findByRole('button', { name: /新增部门/ }));
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: ' Support ' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(departmentsApi.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'Support', parent_id: null, role_codes: [] },
        'csrf-123'
      )
    );
    expect(await screen.findByText('Support')).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers.mock.calls.length).toBeGreaterThan(
        1
      )
    );
    await waitFor(() =>
      expect(screen.queryByLabelText('部门名称')).not.toBeInTheDocument()
    );
  });
  test('allows changing the parent when creating from the root', async () => {
    departmentsApi.createSettingsDepartment.mockResolvedValue({ id: 'child' });
    renderPanel(true, true);
    fireEvent.click(await screen.findByRole('button', { name: /新增部门/ }));
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: 'Support' }
    });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '上级部门' }));
    const [option] = await screen.findAllByText((_, element) =>
      Boolean(
        element?.matches('.ant-select-item-option-content') &&
        element.textContent === 'Engineering'
      )
    );
    fireEvent.click(option);
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(departmentsApi.createSettingsDepartment).toHaveBeenCalledWith(
        { name: 'Support', parent_id: 'department-1', role_codes: [] },
        'csrf-123'
      )
    );
  });
  test('defaults to the selected parent and preserves the draft on failure for a successful retry', async () => {
    departmentsApi.createSettingsDepartment
      .mockRejectedValueOnce(new Error('create rejected'))
      .mockResolvedValueOnce({ id: 'child' });
    renderPanel(true, true);
    fireEvent.click(await screen.findByText('Engineering'));
    await waitFor(() =>
      expect(membersApi.fetchSettingsMembers).toHaveBeenLastCalledWith(
        'department-1'
      )
    );
    fireEvent.click(screen.getByRole('button', { name: /新增部门/ }));
    fireEvent.change(screen.getByLabelText('部门名称'), {
      target: { value: 'Child' }
    });
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(await screen.findByText('操作未完成，请重试')).toBeInTheDocument();
    expect(screen.getByLabelText('部门名称')).toHaveValue('Child');
    expect(departmentsApi.createSettingsDepartment).toHaveBeenCalledWith(
      { name: 'Child', parent_id: 'department-1', role_codes: [] },
      'csrf-123'
    );
    const beforeRetry = membersApi.fetchSettingsMembers.mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(departmentsApi.createSettingsDepartment).toHaveBeenCalledTimes(2)
    );
    await waitFor(() =>
      expect(screen.queryByLabelText('部门名称')).not.toBeInTheDocument()
    );
    expect(membersApi.fetchSettingsMembers.mock.calls.length).toBeGreaterThan(
      beforeRetry
    );
  });
  test('department membership saves an explicit null primary for an unassigned user', async () => {
    renderPanel(true);
    const row = await findMemberRow(/user.*User Name.*User Nick/u);
    fireEvent.click(within(row).getByRole('button', { name: /编辑$/ }));
    const dialog = await findProfileDialog('User Name');
    expect(
      within(dialog).getByRole('combobox', { name: '主部门' })
    ).toBeDisabled();
    fireEvent.click(within(dialog).getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(
        departmentsApi.replaceSettingsMemberDepartments
      ).toHaveBeenCalledWith(
        'user-2',
        { department_ids: [], primary_department_id: null },
        'csrf-123'
      )
    );
  });
});
