import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { navigateSpy, fetchCurrentMe, fetchLoginInstances, renderedBlocks } =
  vi.hoisted(() => ({
    navigateSpy: vi.fn(),
    fetchCurrentMe: vi.fn(),
    fetchLoginInstances: vi.fn(),
    renderedBlocks: vi.fn()
  }));

vi.mock('@tanstack/react-router', async () => {
  const actual = await vi.importActual<typeof import('@tanstack/react-router')>(
    '@tanstack/react-router'
  );
  return { ...actual, useNavigate: () => navigateSpy };
});

vi.mock('../api/session', () => ({ fetchCurrentMe, fetchLoginInstances }));

vi.mock('../components/PublicAuthBlock', () => ({
  PublicAuthBlock: (props: {
    instance: { id: string; public_ui_block: string };
    authenticatorSelector: { request: () => void } | null;
    onAuthenticated: (session: {
      csrf_token: string;
      effective_display_role: string;
      current_workspace_id: string;
    }) => Promise<void>;
  }) => {
    renderedBlocks(props.instance, Boolean(props.authenticatorSelector));
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
        {props.authenticatorSelector ? (
          <button onClick={props.authenticatorSelector.request}>
            Request authenticator selector
          </button>
        ) : null}
      </>
    );
  }
}));

import { AppProviders } from '../../../app/AppProviders';
import { useAuthStore } from '../../../state/auth-store';
import { SignInPage } from '../pages/SignInPage';

const passwordInstance = {
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
    fetchCurrentMe.mockReset();
    fetchLoginInstances.mockReset();
    renderedBlocks.mockReset();
    useAuthStore.getState().setAnonymous();
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_id: passwordInstance.id,
      login_instances: [passwordInstance]
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
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_id: passwordInstance.id,
      login_instances: [passwordInstance, qrInstance]
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
      search: { authenticator_id: 'auth-qr' }
    });

    view.rerender(
      <AppProviders>
        <SignInPage authenticatorId="auth-qr" />
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
      search: { authenticator_id: undefined },
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
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_id: passwordInstance.id,
      login_instances: [passwordInstance, qrInstance]
    });

    render(
      <AppProviders>
        <SignInPage authenticatorId="auth-qr" />
      </AppProviders>
    );

    expect(
      await screen.findByRole('button', { name: 'Run auth-qr' })
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Password' })).not.toBeInTheDocument();
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
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_id: passwordInstance.id,
      login_instances: [passwordInstance, qrInstance]
    });

    render(
      <AppProviders>
        <SignInPage authenticatorId="missing-authenticator" />
      </AppProviders>
    );

    expect(await screen.findByRole('button', { name: 'Password' })).toBeInTheDocument();
    await waitFor(() =>
      expect(navigateSpy).toHaveBeenCalledWith({
        to: '/sign-in',
        search: { authenticator_id: undefined },
        replace: true
      })
    );
    expect(renderedBlocks).not.toHaveBeenCalled();
  });

  test('shows a formal unavailable state and no fallback form when none are enabled', async () => {
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_id: '',
      login_instances: []
    });

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'No sign-in option is available.'
    );
    expect(screen.queryByRole('form')).not.toBeInTheDocument();
    expect(renderedBlocks).not.toHaveBeenCalled();
  });

  test('shows a stable error and no fallback form when discovery fails', async () => {
    fetchLoginInstances.mockRejectedValue(new Error('network failed'));
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Failed to load sign-in options. Please refresh the page and try again.'
    );
    expect(screen.queryByRole('form')).not.toBeInTheDocument();
  });
});
