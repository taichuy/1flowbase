import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

const { navigateSpy, fetchCurrentMe, redeemWorkspaceInvitation } = vi.hoisted(
  () => ({
    navigateSpy: vi.fn(),
    fetchCurrentMe: vi.fn(),
    redeemWorkspaceInvitation: vi.fn()
  })
);

vi.mock('@tanstack/react-router', () => ({
  Navigate: (props: { to: string; replace?: boolean }) => {
    navigateSpy(props);
    return <div data-testid="router-navigate">{props.to}</div>;
  }
}));

vi.mock('../../api/session', () => ({
  fetchCurrentMe
}));
vi.mock('../../api/session-actions', () => ({
  redeemWorkspaceInvitation
}));

import { AuthenticatedSignInRedirect } from '../AuthenticatedSignInRedirect';

describe('AuthenticatedSignInRedirect', () => {
  beforeEach(() => {
    resetAuthStore();
    navigateSpy.mockReset();
    fetchCurrentMe.mockReset();
    redeemWorkspaceInvitation.mockReset();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-initial',
      actor: {
        id: 'user-1',
        account: 'alice',
        effective_display_role: 'admin',
        current_workspace_id: 'workspace-old'
      },
      me: {
        id: 'user-1',
        account: 'alice',
        email: 'alice@example.com',
        phone: null,
        nickname: 'Alice',
        name: 'Alice',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'admin',
        permissions: []
      },
      availableRoles: []
    });
  });

  test('redirects immediately to / without calling redeem when invite is absent', () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AuthenticatedSignInRedirect />
        </App>
      </QueryClientProvider>
    );

    expect(redeemWorkspaceInvitation).not.toHaveBeenCalled();
    expect(navigateSpy).toHaveBeenCalledWith({ to: '/', replace: true });
  });

  test('AC-002 redeems invitation for authenticated user, switches workspace, clears query cache, and redirects home', async () => {
    const queryClient = new QueryClient();
    queryClient.setQueryData(['cached-apps'], ['app-old']);
    redeemWorkspaceInvitation.mockResolvedValue({
      actor: {
        id: 'user-1',
        account: 'alice',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-joined'
      },
      session: {
        id: 'session-1',
        user_id: 'user-1',
        tenant_id: 'tenant-joined',
        current_workspace_id: 'workspace-joined',
        active_role_code: 'member'
      },
      available_roles: [
        { code: 'member', name: 'Member', scope_kind: 'workspace' }
      ],
      active_role_permissions: ['applications.read'],
      csrf_token: 'csrf-joined',
      cookie_name: 'flowbase_console_session'
    });
    fetchCurrentMe.mockResolvedValue({
      id: 'user-1',
      account: 'alice',
      email: 'alice@example.com',
      phone: null,
      nickname: 'Alice',
      name: 'Alice',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'member',
      permissions: ['applications.read']
    });

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AuthenticatedSignInRedirect invite="inv_join_new_ws" />
        </App>
      </QueryClientProvider>
    );

    await waitFor(() =>
      expect(redeemWorkspaceInvitation).toHaveBeenCalledWith(
        'inv_join_new_ws',
        'csrf-initial'
      )
    );
    await waitFor(() =>
      expect(screen.getByTestId('router-navigate')).toHaveTextContent('/')
    );
    expect(useAuthStore.getState().csrfToken).toBe('csrf-joined');
    expect(useAuthStore.getState().actor?.current_workspace_id).toBe(
      'workspace-joined'
    );
    expect(queryClient.getQueryData(['cached-apps'])).toBeUndefined();
  });
});
