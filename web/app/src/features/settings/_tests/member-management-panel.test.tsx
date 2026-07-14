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

function renderPanel() {
  return render(
    <AppProviders>
      <MemberManagementPanel
        canManageMembers
        canManageRoleBindings
      />
    </AppProviders>
  );
}

function findMemberRow(name: RegExp) {
  return screen.findByRole('row', { name });
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
    resetAuthStore();
    authenticate();
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
      screen.queryByRole('columnheader', { name: '角色' })
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

        const dialog = await screen.findByRole('dialog', {
          name: /编辑用户资料/
        });
        expect(
          within(dialog).getByRole('combobox', { name: '角色' })
        ).toBeInTheDocument();

        fireEvent.change(within(dialog).getByLabelText('姓名'), {
          target: { value: 'Root Next' }
        });
        fireEvent.mouseDown(
          within(dialog).getByRole('combobox', { name: '角色' })
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
});
