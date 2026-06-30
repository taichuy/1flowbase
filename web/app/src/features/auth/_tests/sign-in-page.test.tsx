import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { navigateSpy, signInWithPassword, fetchCurrentMe, fetchLoginInstances } =
  vi.hoisted(() => ({
    navigateSpy: vi.fn(),
    signInWithPassword: vi.fn(),
    fetchCurrentMe: vi.fn(),
    fetchLoginInstances: vi.fn()
  }));

vi.mock('@tanstack/react-router', async () => {
  const actual = await vi.importActual<typeof import('@tanstack/react-router')>(
    '@tanstack/react-router'
  );

  return {
    ...actual,
    useNavigate: () => navigateSpy
  };
});

vi.mock('../api/session', () => ({
  signInWithPassword,
  fetchCurrentMe,
  fetchLoginInstances
}));

import { AppProviders } from '../../../app/AppProviders';
import { useAuthStore } from '../../../state/auth-store';
import { SignInPage } from '../pages/SignInPage';

describe('SignInPage', () => {
  beforeEach(() => {
    window.history.pushState({}, '', '/sign-in');
    window.localStorage.clear();
    navigateSpy.mockReset();
    signInWithPassword.mockReset();
    fetchCurrentMe.mockReset();
    fetchLoginInstances.mockReset();
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_name: 'password-local',
      login_instances: [
        {
          name: 'password-local',
          auth_type: 'password-local',
          title: 'Password',
          description: 'Local password login',
          sort_order: 0,
          flow: 'password',
          sign_in_path: '/api/public/auth/providers/password-local/sign-in',
          public_options: {}
        }
      ]
    });
    useAuthStore.getState().setAnonymous();
  });

  test('submits account/password and redirects to home on success', async () => {
    signInWithPassword.mockResolvedValue({
      csrf_token: 'csrf-123',
      effective_display_role: 'manager',
      current_workspace_id: 'workspace-1'
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
      effective_display_role: 'manager',
      permissions: ['route_page.view.all']
    });

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    fireEvent.change(await screen.findByLabelText('Account'), {
      target: { value: 'root' }
    });
    fireEvent.change(screen.getByPlaceholderText('Enter password'), {
      target: { value: 'change-me' }
    });
    fireEvent.click(screen.getByRole('button', { name: /Sign in/ }));

    await waitFor(() =>
      expect(signInWithPassword).toHaveBeenCalledWith({
        identifier: 'root',
        password: 'change-me'
      })
    );
    await waitFor(() => expect(fetchCurrentMe).toHaveBeenCalled());
    await waitFor(() => expect(navigateSpy).toHaveBeenCalledWith({ to: '/' }));

    expect(useAuthStore.getState()).toEqual(
      expect.objectContaining({
        sessionStatus: 'authenticated',
        csrfToken: 'csrf-123',
        actor: expect.objectContaining({
          account: 'root',
          current_workspace_id: 'workspace-1'
        }),
        me: expect.objectContaining({
          name: 'Root'
        })
      })
    );
  });

  test('submits the selected authenticator when multiple login instances exist', async () => {
    fetchLoginInstances.mockResolvedValue({
      default_authenticator_name: 'staff_password',
      login_instances: [
        {
          name: 'staff_password',
          auth_type: 'password-local',
          title: 'Staff Password',
          description: 'Staff login',
          sort_order: 0,
          flow: 'password',
          sign_in_path: '/api/public/auth/providers/password-local/sign-in',
          public_options: {}
        },
        {
          name: 'password-local',
          auth_type: 'password-local',
          title: 'Password',
          description: 'Local password login',
          sort_order: 10,
          flow: 'password',
          sign_in_path: '/api/public/auth/providers/password-local/sign-in',
          public_options: {}
        }
      ]
    });
    signInWithPassword.mockResolvedValue({
      csrf_token: 'csrf-123',
      effective_display_role: 'manager',
      current_workspace_id: 'workspace-1'
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
      effective_display_role: 'manager',
      permissions: ['route_page.view.all']
    });

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByText('Staff Password')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Account'), {
      target: { value: 'root' }
    });
    fireEvent.change(screen.getByPlaceholderText('Enter password'), {
      target: { value: 'change-me' }
    });
    fireEvent.click(screen.getByRole('button', { name: /Sign in/ }));

    await waitFor(() =>
      expect(signInWithPassword).toHaveBeenCalledWith({
        authenticator: 'staff_password',
        identifier: 'root',
        password: 'change-me'
      })
    );
  });

  test('shows a stable error when login instances cannot be loaded', async () => {
    fetchLoginInstances.mockRejectedValue(new Error('network failed'));

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(
      await screen.findByText(
        'Failed to load sign-in options. Please refresh the page and try again.'
      )
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Sign in/ })).toBeDisabled();
  });

  test('defaults the sign-in page copy to English without rendering a language selector', async () => {
    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByLabelText('Account')).toBeInTheDocument();
    expect(screen.getByLabelText('Password')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Sign in/ })).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: /language/i })
    ).not.toBeInTheDocument();
  });

  test('uses the URL language parameter when no cached locale exists', async () => {
    window.history.pushState({}, '', '/sign-in?language=zh');

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByLabelText('\u8d26\u53f7')).toBeInTheDocument();
    expect(screen.getByLabelText('\u5bc6\u7801')).toBeInTheDocument();
  });

  test('uses cached locale preference before the URL language parameter', async () => {
    window.history.pushState({}, '', '/sign-in?language=zh');
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'en_US');

    render(
      <AppProviders>
        <SignInPage />
      </AppProviders>
    );

    expect(await screen.findByLabelText('Account')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Sign in/ })).toBeInTheDocument();
  });
});
