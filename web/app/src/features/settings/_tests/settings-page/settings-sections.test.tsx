import { describe, test, expect } from 'vitest';
import {
  authenticateWithPermissions,
  departmentsApi,
  fireEvent,
  membersApi,
  personalAccessTokensApi,
  renderApp,
  screen,
  useAuthStore,
  waitFor,
  within
} from './settings-sections.fixtures';

describe('SettingsPage', () => {
  test('renders API key for signed-in users without management permissions', async () => {
    authenticateWithPermissions([]);

    renderApp('/settings/api-key-authentication');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/api-key-authentication');
    });
    expect(
      await screen.findByRole('button', { name: /添加/ }, { timeout: 10_000 })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /添加/ })).toBeInTheDocument();
    await waitFor(() => {
      expect(
        personalAccessTokensApi.fetchSettingsPersonalAccessTokens
      ).toHaveBeenCalled();
    });
  });

  test('allows root safe member edits while keeping destructive actions locked', async () => {
    authenticateWithPermissions(['user.view.all', 'user.manage.all'], 'root');
    // OrganizationAccess is backend-owned; a real root actor has these grants.
    departmentsApi.fetchSettingsDepartmentAccess.mockResolvedValue({
      can_list: true,
      can_create: true,
      can_update: true,
      can_delete: true,
      can_assign_roles: true,
      can_replace_member_departments: true
    });
    departmentsApi.fetchSettingsDepartments.mockResolvedValue({
      items: [],
      has_more: false,
      next_cursor: null
    });
    departmentsApi.fetchSettingsMemberRoleOptions.mockResolvedValue([
      { code: 'operator', name: 'Operator' }
    ]);
    departmentsApi.replaceSettingsMemberDepartments.mockResolvedValue(
      undefined
    );
    departmentsApi.replaceSettingsMemberDepartments.mockClear();
    membersApi.updateSettingsMember.mockClear();
    membersApi.replaceSettingsMemberRoles.mockClear();
    membersApi.changeCurrentUserPassword.mockClear();
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
      role_codes: ['root'],
      department_ids: [],
      primary_department_id: null
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
        role_codes: ['root'],
        department_ids: [],
        primary_department_id: null
      },
      {
        id: 'manager-1',
        account: 'manager-1',
        email: 'manager-1@example.com',
        phone: null,
        name: 'Manager 1',
        nickname: 'Manager 1',
        introduction: '',
        default_display_role: 'member',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        role_codes: ['member'],
        department_ids: [],
        primary_department_id: null
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
      throw new Error('Expected root and regular member rows to be rendered.');
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
    // rc-component shares test heading ids across dialogs; bind the actual
    // modal through its visible title, as the member-panel contract test does.
    const profileTitle = await screen.findByText('编辑用户资料 Root');
    const profileDialog = profileTitle.closest<HTMLElement>('[role="dialog"]');
    if (!profileDialog) throw new Error('Profile title is not inside a dialog');
    await waitFor(() => expect(profileDialog).toBeVisible());
    expect(profileDialog).toHaveAttribute('aria-modal', 'true');
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
      await within(profileDialog).findByRole('combobox', { name: '直接角色' })
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
      expect(membersApi.replaceSettingsMemberRoles).toHaveBeenCalledWith(
        'user-1',
        { role_codes: ['root'] },
        'csrf-123'
      );
      expect(
        departmentsApi.replaceSettingsMemberDepartments
      ).toHaveBeenCalledWith(
        'user-1',
        { department_ids: [], primary_department_id: null },
        'csrf-123'
      );
      expect(profileDialog).not.toBeInTheDocument();
    });

    fireEvent.click(within(rootRow).getByRole('button', { name: /重置密码$/ }));
    const currentPasswordInput = await screen.findByLabelText('当前密码');
    const passwordDialog =
      currentPasswordInput.closest<HTMLElement>('[role="dialog"]');
    if (!passwordDialog)
      throw new Error('Password field is not inside a dialog');
    // Form fields mount before the modal's asynchronous opening is visible.
    await waitFor(() => {
      expect(passwordDialog).toBeVisible();
      expect(within(passwordDialog).getByText('重置密码')).toBeVisible();
      for (const label of ['当前密码', '新密码', '确认新密码']) {
        const input = within(passwordDialog).getByLabelText(label);
        expect(input).toBeVisible();
        expect(input).toBeEnabled();
      }
    });
    expect(passwordDialog).toHaveAttribute('aria-modal', 'true');
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
    await waitFor(() => {
      expect(passwordDialog).not.toBeInTheDocument();
      expect(window.location.pathname).toBe('/sign-in');
      expect(useAuthStore.getState().sessionStatus).toBe('anonymous');
    });
  }, 20_000);
});
