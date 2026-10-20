import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const {
  navigateSpy,
  externalRedirect,
  fetchCurrentMe,
  fetchCurrentSession,
  fetchLoginEntries,
  renderedBlocks,
  publicAuthBlockControl
} = vi.hoisted(() => ({
  navigateSpy: vi.fn(),
  externalRedirect: vi.fn(),
  fetchCurrentMe: vi.fn(),
  fetchCurrentSession: vi.fn(),
  fetchLoginEntries: vi.fn(),
  renderedBlocks: vi.fn(),
  publicAuthBlockControl: {
    suspend: false,
    pending: new Promise<never>(() => undefined)
  }
}));

vi.mock('@tanstack/react-router', async () => {
  const actual = await vi.importActual<typeof import('@tanstack/react-router')>(
    '@tanstack/react-router'
  );
  return { ...actual, useNavigate: () => navigateSpy };
});

vi.mock('../api/session', () => ({
  fetchCurrentMe,
  fetchCurrentSession,
  fetchLoginEntries
}));

vi.mock('../lib/external-login', () => ({
  redirectToExternalLogin: externalRedirect
}));

vi.mock('../components/PublicAuthBlock', () => ({
  PublicAuthBlock: (props: {
    instance: { id: string; public_ui_block: string };
    loginEntrySelector: { request: () => void } | null;
    onAuthenticated: (session: {
      csrf_token: string;
      effective_display_role: string;
      current_workspace_id: string;
    }) => Promise<void>;
  }) => {
    renderedBlocks(props.instance, Boolean(props.loginEntrySelector));
    if (publicAuthBlockControl.suspend) {
      throw publicAuthBlockControl.pending;
    }
    return (
      <>
        <button
          onClick={() =>
            void props.onAuthenticated({
              csrf_token: 'csrf-123',
              effective_display_role: 'member',
              current_workspace_id: 'workspace-1'
            })
          }
        >
          Run {props.instance.id}
        </button>
        {props.loginEntrySelector ? (
          <button onClick={props.loginEntrySelector.request}>
            Request authenticator selector
          </button>
        ) : null}
      </>
    );
  }
}));

vi.mock('../components/BuiltinPasswordSignIn', () => ({
  BuiltinPasswordSignIn: () => <form aria-label="Escape password sign in" />
}));

import { AppProviders } from '../../../app/AppProviders';
import { useAuthStore } from '../../../state/auth-store';
import { SignInPage } from '../pages/SignInPage';

const passwordInstance = {
  entry_action: 'render_ui' as const,
  id: 'auth-password-local',
  auth_type: 'password-local',
  is_builtin: true,
  title: 'Password',
  description: 'Local password login',
  sort_order: 0,
  public_ui_block: 'export default function AuthBlock() { return null; }',
  public_variables: { self_registration_enabled: false }
};

describe('SignInPage', () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.history.pushState({}, '', '/sign-in');
    navigateSpy.mockReset();
    externalRedirect.mockReset();
    fetchCurrentMe.mockReset();
    fetchCurrentSession.mockReset();
    fetchLoginEntries.mockReset();
    renderedBlocks.mockReset();
    publicAuthBlockControl.suspend = false;
    useAuthStore.getState().setAnonymous();
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      login_entries: [passwordInstance]
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
      effective_display_role: 'member',
      permissions: []
    });
    fetchCurrentSession.mockResolvedValue({
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-1'
      },
      session: {
        id: 'session-1',
        user_id: 'user-1',
        tenant_id: 'tenant-1',
        current_workspace_id: 'workspace-1',
        active_role_code: 'member'
      },
      available_roles: [],
      active_role_permissions: [],
      csrf_token: 'csrf-123',
      cookie_name: 'flowbase_console_session'
    });
  });

  test('AC-001 shows the shared loading state while login entries are pending', () => {
    fetchLoginEntries.mockImplementation(() => new Promise(() => undefined));

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(
      screen.getByRole('status', {
        name: 'thinking'
      })
    ).toBeVisible();
    expect(screen.getByText('Powered by 1flowbase')).toBeVisible();
  });

  test('AC-002 keeps the shared loading state while the selected auth block suspends', async () => {
    publicAuthBlockControl.suspend = true;

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    await waitFor(() => expect(renderedBlocks).toHaveBeenCalled());
    expect(
      screen.getByRole('status', {
        name: 'thinking'
      })
    ).toBeVisible();
    expect(screen.getByText('Powered by 1flowbase')).toBeVisible();
  });

  test('mounts the only authenticator Block directly and accepts its session', async () => {
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: 'Run auth-password-local' })
    );

    expect(renderedBlocks).toHaveBeenCalledWith(passwordInstance, false);
    await waitFor(() => expect(fetchCurrentMe).toHaveBeenCalled());
    await waitFor(() => expect(navigateSpy).toHaveBeenCalledWith({ to: '/' }));
    expect(useAuthStore.getState().csrfToken).toBe('csrf-123');
  });

  test('AC-001/002 stores a multi-authenticator selection in the URL and lets the Block return', async () => {
    const qrInstance = {
      ...passwordInstance,
      id: 'auth-qr',
      auth_type: 'qr-code',
      is_builtin: false,
      title: 'QR code',
      sort_order: 10,
      public_ui_block: 'qr block'
    };
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      login_entries: [passwordInstance, qrInstance]
    });

    const view = render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(
      await screen.findByRole('button', { name: 'Password' })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'QR code' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Run auth-password-local' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Run auth-qr' })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'QR code' }));
    expect(navigateSpy).toHaveBeenCalledWith({
      to: '/sign-in',
      search: { login_entry_id: 'auth-qr' }
    });

    view.rerender(
      <AppProviders>
        <SignInPage loginEntryId="auth-qr" />
      </AppProviders>
    );
    expect(
      await screen.findByRole('button', { name: 'Run auth-qr' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Password' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'QR code' })
    ).not.toBeInTheDocument();

    expect(renderedBlocks).toHaveBeenLastCalledWith(qrInstance, true);
    fireEvent.click(
      screen.getByRole('button', { name: 'Request authenticator selector' })
    );
    expect(navigateSpy).toHaveBeenLastCalledWith({
      to: '/sign-in',
      search: { login_entry_id: undefined },
      replace: true
    });

    view.rerender(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );
    expect(
      await screen.findByRole('button', { name: 'Password' })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'QR code' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Run auth-qr' })
    ).not.toBeInTheDocument();
  });

  test('AC-001 restores a valid authenticator from the URL on first render', async () => {
    const qrInstance = {
      ...passwordInstance,
      id: 'auth-qr',
      auth_type: 'qr-code',
      is_builtin: false,
      title: 'QR code',
      sort_order: 10,
      public_ui_block: 'qr block'
    };
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      login_entries: [passwordInstance, qrInstance]
    });

    render(
      <AppProviders>
        <SignInPage loginEntryId="auth-qr" />
      </AppProviders>
    );

    expect(
      await screen.findByRole('button', { name: 'Run auth-qr' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Password' })
    ).not.toBeInTheDocument();
  });

  test('AC-001 clears an invalid URL authenticator and shows the chooser', async () => {
    const qrInstance = {
      ...passwordInstance,
      id: 'auth-qr',
      auth_type: 'qr-code',
      is_builtin: false,
      title: 'QR code',
      sort_order: 10,
      public_ui_block: 'qr block'
    };
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      login_entries: [passwordInstance, qrInstance]
    });

    render(
      <AppProviders>
        <SignInPage loginEntryId="missing-authenticator" />
      </AppProviders>
    );

    expect(
      await screen.findByRole('button', { name: 'Password' })
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(navigateSpy).toHaveBeenCalledWith({
        to: '/sign-in',
        search: { login_entry_id: undefined },
        replace: true
      })
    );
    expect(renderedBlocks).not.toHaveBeenCalled();
  });

  test('AC-002 shows the escape form when no login instances are enabled', async () => {
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: '',
      login_entries: []
    });

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(
      await screen.findByRole('form', { name: 'Escape password sign in' })
    ).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(renderedBlocks).not.toHaveBeenCalled();
  });

  test('AC-001 shows the escape form when login instance discovery fails', async () => {
    fetchLoginEntries.mockRejectedValue(new Error('network failed'));
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(
      await screen.findByRole('form', { name: 'Escape password sign in' })
    ).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

describe('declarative authentication entry actions', () => {
  const external = {
    ...passwordInstance,
    id: 'external-arbitrary-id',
    auth_type: 'example/custom-provider',
    is_builtin: false,
    title: 'External account',
    entry_action: 'external_login' as const
  };
  beforeEach(() => {
    window.history.pushState({}, '', '/sign-in');
    navigateSpy.mockReset();
    externalRedirect.mockReset();
    renderedBlocks.mockReset();
    publicAuthBlockControl.suspend = false;
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      login_entries: [passwordInstance, external]
    });
  });

  test('one chooser click starts external authorization without routing to or mounting a block', async () => {
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );
    const button = await screen.findByRole('button', { name: external.title });
    expect(externalRedirect).not.toHaveBeenCalled();
    fireEvent.click(button);
    fireEvent.click(button);
    expect(externalRedirect).toHaveBeenCalledExactlyOnceWith(external.id);
    expect(navigateSpy).not.toHaveBeenCalled();
    expect(renderedBlocks).not.toHaveBeenCalled();
    expect(button).toBeDisabled();
    // Back-forward cache restores the page and allows another intentional click.
    fireEvent(window, new Event('pageshow'));
    expect(button).toBeEnabled();
    fireEvent.click(button);
    expect(externalRedirect).toHaveBeenCalledTimes(2);
  });

  test.each(['single', 'deep-link', 'cancelled', 'failed'])(
    '%s waits for an explicit click and never redirects on mount',
    async (mode) => {
      if (mode === 'single')
        fetchLoginEntries.mockResolvedValue({
          default_login_entry_id: external.id,
          login_entries: [external]
        });
      if (mode === 'cancelled' || mode === 'failed')
        window.history.pushState(
          {},
          '',
          '/sign-in?auth_error=' +
            (mode === 'cancelled' ? 'cancelled' : 'external_auth_failed')
        );
      render(
        <AppProviders>
          <SignInPage
            loginEntryId={mode === 'deep-link' ? external.id : undefined}
          />
        </AppProviders>
      );
      const button = await screen.findByRole('button', {
        name: external.title
      });
      expect(externalRedirect).not.toHaveBeenCalled();
      expect(renderedBlocks).not.toHaveBeenCalled();
      if (mode === 'cancelled' || mode === 'failed')
        expect(screen.getByRole('alert')).toBeVisible();
      fireEvent.click(button);
      expect(externalRedirect).toHaveBeenCalledExactlyOnceWith(external.id);
    }
  );

  test('navigation failure leaves an error and a working retry button', async () => {
    externalRedirect.mockImplementationOnce(() => {
      throw new Error('blocked');
    });
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );
    const button = await screen.findByRole('button', { name: external.title });
    fireEvent.click(button);
    expect(screen.getByRole('alert')).toBeVisible();
    expect(button).toBeEnabled();
    fireEvent.click(button);
    expect(externalRedirect).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  test('an unlinked identity explains recovery in the original account and never retries OAuth automatically', async () => {
    window.history.pushState(
      {},
      '',
      '/sign-in?auth_error=identity_unlinked&language=zh'
    );
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('该 GitHub 身份已解绑');
    expect(alert).toHaveTextContent('原本站账号');
    expect(alert).toHaveTextContent('恢复绑定');
    expect(alert).toHaveTextContent('不会自动创建新账号');
    expect(externalRedirect).not.toHaveBeenCalled();
  });
});
