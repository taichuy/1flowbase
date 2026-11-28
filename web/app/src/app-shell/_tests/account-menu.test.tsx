import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import type { MenuProps } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { ConsoleMe, ConsoleSessionActor } from '@1flowbase/api-client';

import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import { AccountMenu } from '../AccountMenu';
import {
  createAccountMenuClickHandler,
  selectAccountLabel
} from '../account-menu-actions';
import { createAccountMenuItems } from '../account-menu-items';

const sessionApi = vi.hoisted(() => ({
  fetchAccessibleWorkspaces: vi.fn(),
  fetchCurrentMe: vi.fn(),
  signOut: vi.fn(),
  switchActiveRole: vi.fn(),
  switchWorkspace: vi.fn()
}));

vi.mock('../../features/auth/api/session', () => sessionApi);
vi.mock('../../features/auth/api/session-actions', () => sessionApi);

const actor: ConsoleSessionActor = {
  id: 'user-1',
  current_workspace_id: 'workspace-1',
  account: 'root',
  effective_display_role: 'root'
};

function buildMe(overrides: Partial<ConsoleMe> = {}): ConsoleMe {
  return {
    id: 'user-1',
    account: 'root',
    email: 'root@example.com',
    phone: null,
    nickname: '',
    name: 'Root User',
    avatar_url: null,
    introduction: '',
    effective_display_role: 'root',
    policy_fingerprint: 'fixture-policy',

    ...overrides
  };
}

function menuClick(
  key: string
): Parameters<NonNullable<MenuProps['onClick']>>[0] {
  return { key } as Parameters<NonNullable<MenuProps['onClick']>>[0];
}

beforeEach(() => {
  resetAuthStore();
  sessionApi.fetchAccessibleWorkspaces.mockReset();
  sessionApi.fetchCurrentMe.mockReset();
  sessionApi.signOut.mockReset();
  sessionApi.switchActiveRole.mockReset();
  sessionApi.switchWorkspace.mockReset();
  sessionApi.fetchAccessibleWorkspaces.mockResolvedValue([]);
  sessionApi.signOut.mockResolvedValue(undefined);
});

describe('createAccountMenuItems', () => {
  test('uses native Ant menu icons for account actions', () => {
    const items = createAccountMenuItems() ?? [];
    const accountItem = items[0];
    const children = accountItem ? Reflect.get(accountItem, 'children') : [];
    const rawChildren =
      accountItem && typeof accountItem === 'object' && Array.isArray(children)
        ? children
        : [];

    expect(
      rawChildren.flatMap((item: unknown) => {
        if (
          !item ||
          typeof item !== 'object' ||
          !('key' in item) ||
          typeof item.key !== 'string' ||
          !('label' in item) ||
          typeof item.label !== 'string'
        ) {
          return [];
        }

        return [
          {
            key: item.key,
            label: item.label,
            hasIcon: 'icon' in item && Boolean(item.icon)
          }
        ];
      })
    ).toEqual([
      { key: 'profile', label: '个人资料', hasIcon: true },
      { key: 'active-role', label: '当前角色', hasIcon: true },
      { key: 'sign-out', label: '退出登录', hasIcon: true }
    ]);
  });

  test('marks the active role and exposes the remaining bound roles', () => {
    const items =
      createAccountMenuItems(
        'Root',
        [
          { code: 'root', name: 'Root', scope_kind: 'system' },
          { code: 'manager', name: 'Manager', scope_kind: 'workspace' }
        ],
        'root'
      ) ?? [];
    const children = Reflect.get(items[0] ?? {}, 'children') as unknown[];
    const roleItem = children.find(
      (item) =>
        item &&
        typeof item === 'object' &&
        Reflect.get(item, 'key') === 'active-role'
    );
    const roles = Reflect.get(roleItem ?? {}, 'children') as Array<
      Record<string, unknown>
    >;

    expect(
      roles.map((role) => ({
        key: role.key,
        label: role.label,
        disabled: role.disabled
      }))
    ).toEqual([
      { key: 'role:root', label: 'Root', disabled: true },
      { key: 'role:manager', label: 'Manager', disabled: false }
    ]);
  });

  test('AC-002 hides workspace submenu when <= 1 workspace is accessible and shows it when > 1', () => {
    const singleItems =
      createAccountMenuItems(
        'Ada',
        [],
        'admin',
        null,
        [
          {
            id: 'workspace-1',
            name: 'Alpha Workspace',
            logo_url: null,
            introduction: '',
            is_current: true
          }
        ],
        'workspace-1'
      ) ?? [];
    const singleChildren = Reflect.get(
      singleItems[0] ?? {},
      'children'
    ) as Array<Record<string, unknown>>;
    expect(
      singleChildren.some((item) => item?.key === 'active-workspace')
    ).toBe(false);

    const multiItems =
      createAccountMenuItems(
        'Ada',
        [],
        'admin',
        null,
        [
          {
            id: 'workspace-1',
            name: 'Alpha Workspace',
            logo_url: null,
            introduction: '',
            is_current: true
          },
          {
            id: 'workspace-2',
            name: 'Beta Workspace',
            logo_url: null,
            introduction: '',
            is_current: false
          }
        ],
        'workspace-1'
      ) ?? [];
    const multiChildren = Reflect.get(multiItems[0] ?? {}, 'children') as Array<
      Record<string, unknown>
    >;
    const workspaceSubmenu = multiChildren.find(
      (item) => item?.key === 'active-workspace'
    );
    expect(workspaceSubmenu).toBeDefined();
    const workspaceOptions = Reflect.get(
      workspaceSubmenu ?? {},
      'children'
    ) as Array<Record<string, unknown>>;
    expect(
      workspaceOptions.map((option) => ({
        key: option.key,
        label: option.label,
        disabled: option.disabled
      }))
    ).toEqual([
      {
        key: 'workspace:workspace-1',
        label: 'Alpha Workspace',
        disabled: true
      },
      {
        key: 'workspace:workspace-2',
        label: 'Beta Workspace',
        disabled: false
      }
    ]);
  });
});

describe('AccountMenuBase', () => {
  test('selects nickname, profile name, actor account, then fallback for the account label', () => {
    expect(
      selectAccountLabel({
        me: buildMe({ name: 'Ada Lovelace', nickname: 'Ada' }),
        actor
      })
    ).toBe('Ada');
    expect(
      selectAccountLabel({
        me: buildMe({ name: 'Ada Lovelace', nickname: '' }),
        actor
      })
    ).toBe('Ada Lovelace');
    expect(selectAccountLabel({ me: null, actor })).toBe('root');
    expect(selectAccountLabel({ me: null, actor: null })).toBe('用户');
  });

  test('navigates to the profile page from the account menu action', () => {
    const navigateTo = vi.fn();
    const onClick = createAccountMenuClickHandler({
      csrfToken: 'csrf-token',
      setAnonymous: vi.fn(),
      navigateTo
    });

    onClick?.(menuClick('profile'));

    expect(navigateTo).toHaveBeenCalledWith('/me');
  });

  test('signs out with csrf token, resets auth state, and redirects to sign in', async () => {
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor,
      me: null
    });
    const navigateTo = vi.fn();
    const onClick = createAccountMenuClickHandler({
      csrfToken: 'csrf-token',
      setAnonymous: useAuthStore.getState().setAnonymous,
      navigateTo
    });

    onClick?.(menuClick('sign-out'));

    await waitFor(() => {
      expect(sessionApi.signOut).toHaveBeenCalledWith('csrf-token');
      expect(navigateTo).toHaveBeenCalledWith('/sign-in');
    });
    expect(useAuthStore.getState().sessionStatus).toBe('anonymous');
  });

  test('switches active role and stores the projected me response', async () => {
    const queryClient = new QueryClient();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor,
      me: buildMe({ policy_fingerprint: 'root-policy' }),
      availableRoles: [
        { code: 'root', name: 'Root', scope_kind: 'system' },
        { code: 'manager', name: 'Manager', scope_kind: 'workspace' }
      ]
    });
    sessionApi.fetchAccessibleWorkspaces.mockResolvedValue([]);
    sessionApi.switchActiveRole.mockResolvedValue({
      actor: { ...actor, effective_display_role: 'manager' },
      csrf_token: 'next-csrf',
      policy_fingerprint: 'manager-policy',
      available_roles: [{ code: 'manager', name: 'Manager', scope_kind: 'workspace' }]
    });
    const nextMe = buildMe({
      effective_display_role: 'manager',
      policy_fingerprint: 'manager-policy'
    });
    sessionApi.fetchCurrentMe.mockResolvedValue(nextMe);

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AccountMenu />
        </App>
      </QueryClientProvider>
    );

    fireEvent.mouseEnter(screen.getByLabelText('Root User'));
    fireEvent.mouseEnter(await screen.findByText('当前角色'));
    fireEvent.click(await screen.findByText('Manager'));

    await waitFor(() => expect(useAuthStore.getState().me).toEqual(nextMe));
    expect(sessionApi.switchActiveRole).toHaveBeenCalledWith('manager', 'csrf-token');
    expect(sessionApi.fetchCurrentMe).toHaveBeenCalledTimes(1);
    expect(useAuthStore.getState().csrfToken).toBe('next-csrf');
  });

  test('AC-002 switches workspace, refreshes me and csrf, clears query cache, and returns home', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    queryClient.setQueryData(['stale-workspace-data'], { items: [1] });
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor,
      me: buildMe(),
      availableRoles: [{ code: 'admin', name: 'Admin', scope_kind: 'workspace' }]
    });
    sessionApi.fetchAccessibleWorkspaces.mockResolvedValue([
      {
        id: 'workspace-1',
        name: 'Alpha Workspace',
        logo_url: null,
        introduction: '',
        is_current: true
      },
      {
        id: 'workspace-2',
        name: 'Beta Workspace',
        logo_url: null,
        introduction: '',
        is_current: false
      }
    ]);
    sessionApi.switchWorkspace.mockResolvedValue({
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-2'
      },
      session: {
        id: 'session-1',
        user_id: 'user-1',
        tenant_id: 'tenant-2',
        current_workspace_id: 'workspace-2',
        active_role_code: 'member'
      },
      available_roles: [
        { code: 'member', name: 'Member', scope_kind: 'workspace' }
      ],
      policy_fingerprint: 'fixture-policy',
      csrf_token: 'csrf-token-ws2',
      cookie_name: 'flowbase_console_session'
    });
    sessionApi.fetchCurrentMe.mockResolvedValue(
      buildMe({
        effective_display_role: 'member',
        policy_fingerprint: 'fixture-policy'
      })
    );

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AccountMenu />
        </App>
      </QueryClientProvider>
    );

    await waitFor(() =>
      expect(sessionApi.fetchAccessibleWorkspaces).toHaveBeenCalledTimes(1)
    );
    fireEvent.mouseEnter(screen.getByLabelText('Root User'));
    const workspaceSubmenu = await screen.findByText('工作空间');
    fireEvent.mouseEnter(workspaceSubmenu);
    const betaOption = await screen.findByText('Beta Workspace');
    fireEvent.click(betaOption);

    await waitFor(() => {
      expect(sessionApi.switchWorkspace).toHaveBeenCalledWith(
        'workspace-2',
        'csrf-token'
      );
      expect(useAuthStore.getState().csrfToken).toBe('csrf-token-ws2');
      expect(useAuthStore.getState().actor?.current_workspace_id).toBe(
        'workspace-2'
      );
    });
    expect(queryClient.getQueryData(['stale-workspace-data'])).toBeUndefined();
  });
});
