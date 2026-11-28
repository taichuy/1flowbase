import { StrictMode } from 'react';
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
import { resetAuthSessionDiscovery } from '../api/auth-session-discovery';

describe('AuthBootstrap', () => {
  beforeEach(() => {
    window.history.pushState({}, '', '/');
    fetchCurrentSession.mockReset();
    fetchCurrentMe.mockReset();
    resetAuthSessionDiscovery();
    resetAuthStore();
  });

  test('starts session and me discovery concurrently on non-sign-in routes before either settles', async () => {
    let resolveSession!: (value: unknown) => void;
    let resolveMe!: (value: unknown) => void;
    fetchCurrentSession.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveSession = resolve;
        })
    );
    fetchCurrentMe.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveMe = resolve;
        })
    );

    render(
      <AppProviders>
        <AuthBootstrap>
          <div>booted app</div>
        </AuthBootstrap>
      </AppProviders>
    );

    await waitFor(() => {
      expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
      expect(fetchCurrentMe).toHaveBeenCalledTimes(1);
    });

    resolveSession({
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
      policy_fingerprint: 'fixture-policy',
      csrf_token: 'csrf-123',
      cookie_name: 'flowbase_console_session'
    });
    resolveMe({
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      policy_fingerprint: 'fixture-policy'
    });

    await waitFor(() =>
      expect(useAuthStore.getState().sessionStatus).toBe('authenticated')
    );
    expect(useAuthStore.getState().me?.account).toBe('root');
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
      policy_fingerprint: 'fixture-policy',
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
      policy_fingerprint: 'fixture-policy'
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

  test('StrictMode reuses discovery and ignores the abandoned bootstrap effect without providers', async () => {
    fetchCurrentSession.mockRejectedValue({ status: 401 });
    fetchCurrentMe.mockRejectedValue({ status: 401 });
    render(
      <StrictMode>
        <AuthBootstrap>
          <div>public runtime</div>
        </AuthBootstrap>
      </StrictMode>
    );
    await waitFor(() =>
      expect(useAuthStore.getState().sessionStatus).toBe('anonymous')
    );
    expect(fetchCurrentSession).toHaveBeenCalledTimes(1);
    expect(fetchCurrentMe).toHaveBeenCalledTimes(1);
  });

  test('marks the app anonymous when session restore returns 401 and skips me on /sign-in', async () => {
    window.history.pushState({}, '', '/sign-in');
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
