import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { fetchCurrentSession, fetchCurrentMe } = vi.hoisted(() => ({
  fetchCurrentSession: vi.fn(),
  fetchCurrentMe: vi.fn()
}));

vi.mock('../api/session', () => ({
  fetchCurrentSession,
  fetchCurrentMe
}));

import { AppProviders } from '../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { AuthBootstrap } from '../components/AuthBootstrap';

describe('AuthBootstrap', () => {
  beforeEach(() => {
    fetchCurrentSession.mockReset();
    fetchCurrentMe.mockReset();
    resetAuthStore();
  });

  test('hydrates actor and me when session restore succeeds', async () => {
    fetchCurrentSession.mockResolvedValue({
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      session: {
        id: 'session-1',
        user_id: 'user-1',
        tenant_id: 'tenant-1',
        current_workspace_id: 'workspace-1',
        active_role_code: 'root'
      },
      available_roles: [{ code: 'root', name: 'Root', scope_kind: 'system' }],
      active_role_permissions: [],
      csrf_token: 'csrf-123',
      cookie_name: 'flowbase_console_session'
    });
    fetchCurrentMe.mockResolvedValue({
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: []
    });

    render(
      <AppProviders>
        <AuthBootstrap>
          <div>booted app</div>
        </AuthBootstrap>
      </AppProviders>
    );

    await waitFor(() =>
      expect(useAuthStore.getState().sessionStatus).toBe('authenticated')
    );
    expect(fetchCurrentMe).toHaveBeenCalled();
    expect(useAuthStore.getState().me?.account).toBe('root');
  });

  test('marks the app anonymous when session restore returns 401', async () => {
    fetchCurrentSession.mockRejectedValue({
      status: 401
    });

    render(
      <AppProviders>
        <AuthBootstrap>
          <div>booted app</div>
        </AuthBootstrap>
      </AppProviders>
    );

    await waitFor(() =>
      expect(useAuthStore.getState().sessionStatus).toBe('anonymous')
    );
    expect(fetchCurrentMe).not.toHaveBeenCalled();
  });
});
