const { systemMetadata } = vi.hoisted(() => ({
  systemMetadata: {
    logo_url: '/icon.svg',
    system_name: 'Test system',
    hide_login_tagline: false,
    system_description: 'Configured description',
    filing_html:
      '<a href="https://beian.miit.gov.cn/" data-free="yes">备案号</a>',
    page_title: undefined as string | undefined,
    login_hero_title: undefined as string | undefined,
    login_hero_tagline: undefined as string | null | undefined,
    hide_powered_by: undefined as boolean | undefined
  }
}));
vi.mock('../lib/public-system-metadata', () => ({
  usePublicSystemMetadata: () => ({ data: systemMetadata })
}));
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';

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

vi.mock('../lib/anonymous-navigation', async () => {
  const actual = await vi.importActual<
    typeof import('../lib/anonymous-navigation')
  >('../lib/anonymous-navigation');
  return { ...actual, navigatePublicRoute: navigateSpy };
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
  // Transform the real lazy provider graph outside per-interaction waits.
  // The mocked block still suspends below, exercising the actual fallback.
  beforeAll(async () => {
    await import('../components/public-runtime/CustomAuthRuntime');
  });
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage loginEntryId="auth-qr" />
      </>
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
      <>
        <SignInPage />
      </>
    );
    expect(
      await screen.findByRole('button', { name: 'Password' })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'QR code' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Run auth-qr' })
    ).not.toBeInTheDocument();
  });

  test('changing selection discards a pending authentication result from the old entry', async () => {
    const qr = {
      ...passwordInstance,
      id: 'auth-qr',
      auth_type: 'qr-code',
      public_ui_block: 'qr block'
    };
    fetchLoginEntries.mockResolvedValue({
      login_entries: [passwordInstance, qr]
    });
    const currentMe = await fetchCurrentMe();
    let resolveMe!: (me: typeof currentMe) => void;
    fetchCurrentMe.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMe = resolve;
        })
    );
    const view = render(<SignInPage loginEntryId={passwordInstance.id} />);
    fireEvent.click(
      await screen.findByRole('button', { name: 'Run auth-password-local' })
    );
    expect(resolveMe).toBeTypeOf('function');

    view.rerender(<SignInPage loginEntryId={qr.id} />);
    expect(
      await screen.findByRole('button', { name: 'Run auth-qr' })
    ).toBeInTheDocument();
    await act(async () => resolveMe(currentMe));
    expect(navigateSpy).not.toHaveBeenCalledWith({ to: '/' });
    expect(useAuthStore.getState().csrfToken).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Run auth-qr' }));
    await waitFor(() =>
      expect(navigateSpy).toHaveBeenCalledExactlyOnceWith({ to: '/' })
    );
    expect(useAuthStore.getState().csrfToken).toBe('csrf-123');
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
      <>
        <SignInPage loginEntryId="auth-qr" />
      </>
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
      <>
        <SignInPage loginEntryId="missing-authenticator" />
      </>
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage />
      </>
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
      <>
        <SignInPage />
      </>
    );
    const button = await screen.findByRole('button', { name: external.title });
    expect(externalRedirect).not.toHaveBeenCalled();
    fireEvent.click(button);
    fireEvent.click(button);
    expect(externalRedirect).toHaveBeenCalledExactlyOnceWith(external.id);
    expect(navigateSpy).not.toHaveBeenCalled();
    expect(renderedBlocks).not.toHaveBeenCalled();
    expect(screen.getByRole('status', { name: 'thinking' })).toBeVisible();
    expect(button).toBeVisible();
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByRole('button', { name: 'Password' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Password' })).toBeDisabled();
    expect(button.closest('.auth-sign-in-panel')).toHaveAttribute('inert');
    expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
      'auth-sign-in-loading-overlay'
    );
    // Back-forward cache restores the page and allows another intentional click.
    fireEvent(window, new Event('pageshow'));
    const restoredButton = screen.getByRole('button', { name: external.title });
    expect(restoredButton).toBeEnabled();
    expect(restoredButton.closest('.auth-sign-in-panel')).not.toHaveAttribute(
      'inert'
    );
    expect(
      screen.queryByRole('status', { name: 'thinking' })
    ).not.toBeInTheDocument();
    fireEvent.click(restoredButton);
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
        <>
          <SignInPage
            loginEntryId={mode === 'deep-link' ? external.id : undefined}
          />
        </>
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
      expect(screen.getByRole('status', { name: 'thinking' })).toBeVisible();
      expect(button).toBeVisible();
      expect(button).toBeDisabled();
      expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
        'auth-sign-in-loading-overlay'
      );
      if (mode === 'cancelled' || mode === 'failed')
        expect(screen.getByRole('alert')).toBeVisible();
    }
  );

  test('navigation failure leaves an error and a working retry button', async () => {
    externalRedirect.mockImplementationOnce(() => {
      throw new Error('blocked');
    });
    render(
      <>
        <SignInPage />
      </>
    );
    const button = await screen.findByRole('button', { name: external.title });
    fireEvent.click(button);
    expect(screen.getByRole('alert')).toBeVisible();
    const restoredButton = screen.getByRole('button', { name: external.title });
    expect(restoredButton).toBeEnabled();
    expect(restoredButton.closest('.auth-sign-in-panel')).not.toHaveAttribute(
      'inert'
    );
    expect(
      screen.queryByRole('status', { name: 'thinking' })
    ).not.toBeInTheDocument();
    fireEvent.click(restoredButton);
    expect(externalRedirect).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByRole('status', { name: 'thinking' })).toBeVisible();
  });
  test('an unlinked identity explains recovery in the original account and never retries OAuth automatically', async () => {
    window.history.pushState(
      {},
      '',
      '/sign-in?auth_error=identity_unlinked&language=zh'
    );
    render(
      <>
        <SignInPage />
      </>
    );
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('该 GitHub 身份已解绑');
    expect(alert).toHaveTextContent('原本站账号');
    expect(alert).toHaveTextContent('恢复绑定');
    expect(alert).toHaveTextContent('不会自动创建新账号');
    expect(externalRedirect).not.toHaveBeenCalled();
  });
});

test('renders raw registration HTML below the powered-by footer', async () => {
  window.localStorage.clear();
  window.history.replaceState({}, '', '/sign-in?language=zh');
  render(
    <>
      <SignInPage />
    </>
  );
  expect(await screen.findByText('1flowbase', { exact: true })).toBeVisible();
  expect(document.querySelector('.hero-slogan-line')).toHaveTextContent(
    '对话即是壁垒，AI应用原生底座'
  );
  expect(screen.queryByText('Configured description')).not.toBeInTheDocument();
  const link = await screen.findByRole('link', { name: '备案号' });
  expect(link).toHaveAttribute('href', 'https://beian.miit.gov.cn/');
  expect(link).toHaveAttribute('data-free', 'yes');
  expect(
    screen.getByText('Powered by 1flowbase').closest('span')?.nextElementSibling
  ).toHaveTextContent('备案号');
});

test('the hide switch removes the fixed login tagline while keeping sign-in and filing available', async () => {
  const view = render(
    <>
      <SignInPage />
    </>
  );
  expect(await screen.findByText('1flowbase', { exact: true })).toBeVisible();
  systemMetadata.hide_login_tagline = true;
  try {
    view.rerender(
      <>
        <SignInPage />
      </>
    );
    expect(document.querySelector('.hero-text-block')).not.toBeInTheDocument();
    expect(document.querySelector('.hero-slogan-line')).not.toBeInTheDocument();
    expect(document.querySelector('.auth-sign-in-panel')).toBeInTheDocument();
    expect(screen.getByText('Powered by 1flowbase')).toBeVisible();
    expect(screen.getByRole('link', { name: '备案号' })).toBeVisible();
    systemMetadata.hide_login_tagline = false;
    view.rerender(
      <>
        <SignInPage />
      </>
    );
    expect(await screen.findByText('1flowbase', { exact: true })).toBeVisible();
  } finally {
    systemMetadata.hide_login_tagline = false;
  }
});

test('commercial metadata customizes page title, hero title/tagline, and hides powered-by without affecting filing or hide_login_tagline', async () => {
  systemMetadata.page_title = 'Acme AI Platform';
  systemMetadata.login_hero_title = 'Acme Native';
  systemMetadata.login_hero_tagline = '专属企业原生底座';
  systemMetadata.hide_powered_by = true;
  systemMetadata.hide_login_tagline = false;
  try {
    const view = render(<SignInPage />);
    expect(
      await screen.findByText('Acme Native', { exact: true })
    ).toBeVisible();
    expect(document.querySelector('.hero-slogan-line')).toHaveTextContent(
      '专属企业原生底座'
    );
    expect(document.title).toBe('Acme AI Platform');
    expect(screen.queryByText('Powered by 1flowbase')).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '备案号' })).toBeVisible();

    systemMetadata.hide_login_tagline = true;
    view.rerender(<SignInPage />);
    expect(document.querySelector('.hero-text-block')).not.toBeInTheDocument();
    expect(screen.queryByText('Powered by 1flowbase')).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '备案号' })).toBeVisible();
  } finally {
    systemMetadata.page_title = undefined;
    systemMetadata.login_hero_title = undefined;
    systemMetadata.login_hero_tagline = undefined;
    systemMetadata.hide_powered_by = undefined;
    systemMetadata.hide_login_tagline = false;
  }
});

test('AC-002 ?invite= enables invited registration and forwards invitationToken to external OAuth', async () => {
  const githubEntry = {
    ...passwordInstance,
    id: 'auth-github',
    auth_type: 'github-oauth',
    entry_action: 'external_login' as const,
    is_builtin: false,
    title: 'GitHub',
    sort_order: 10
  };
  fetchLoginEntries.mockResolvedValue({
    default_login_entry_id: passwordInstance.id,
    onboarding_mode: 'invite_only',
    login_entries: [passwordInstance, githubEntry]
  });
  window.history.pushState({}, '', '/sign-in?invite=inv_team_42');

  const view = render(<SignInPage loginEntryId={passwordInstance.id} />);
  expect(await screen.findByTestId('sign-in-invitation-banner')).toBeVisible();
  await waitFor(() =>
    expect(renderedBlocks).toHaveBeenCalledWith(
      expect.objectContaining({
        id: passwordInstance.id,
        public_variables: { self_registration_enabled: true }
      }),
      true
    )
  );

  view.rerender(<SignInPage />);
  fireEvent.click(await screen.findByRole('button', { name: 'GitHub' }));
  expect(externalRedirect).toHaveBeenCalledWith(
    'auth-github',
    undefined,
    'inv_team_42'
  );
});

test.each([
  'invite_only',
  'reject',
  'default_workspace',
  'personal_workspace'
] as const)(
  'AC-013 %s does not render a manual invitation field',
  async (onboarding_mode) => {
    fetchLoginEntries.mockResolvedValue({
      default_login_entry_id: passwordInstance.id,
      onboarding_mode,
      login_entries: [passwordInstance]
    });
    window.history.pushState({}, '', '/sign-in');

    render(<SignInPage />);
    await waitFor(() => expect(renderedBlocks).toHaveBeenCalled());
    expect(
      screen.queryByLabelText(/邀请码|Invitation code/i)
    ).not.toBeInTheDocument();
  }
);
