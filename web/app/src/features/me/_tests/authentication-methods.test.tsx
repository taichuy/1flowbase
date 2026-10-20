import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { AuthenticationMethodsPanel } from '../components/AuthenticationMethodsPanel';
import type { ConsoleAuthenticationMethods } from '../api/me';

const mocks = vi.hoisted(() => ({
  fetch: vi.fn(),
  authorize: vi.fn(),
  unlink: vi.fn(),
  change: vi.fn(),
  switchAccount: vi.fn()
}));
vi.mock('../api/me', () => ({
  authenticationMethodsQueryKey: ['me', 'authentication-methods'],
  fetchMyAuthenticationMethods: mocks.fetch,
  authorizeMyExternalIdentity: mocks.authorize,
  unlinkMyExternalIdentity: mocks.unlink
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { entry?: string }) =>
      values?.entry ? `${key} ${values.entry}` : key
  })
}));

function fixture(): ConsoleAuthenticationMethods {
  return {
    password_configured: false,
    password_usable: false,
    password_setup_verified: false,
    password_verification_entries: [
      { id: 'github-entry', title: 'GitHub production' }
    ],
    providers: [
      {
        auth_type: '1flowbase/github-login',
        display_name: 'GitHub',
        provider_available: true,
        login_entries: [
          { id: 'github-entry', title: 'GitHub production' },
          { id: 'github-backup', title: 'GitHub backup' }
        ],
        bindings: [
          {
            identity_id: 'github-identity',
            state: 'bound',
            subject_type: 'github_user_id',
            subject_value: '12345',
            issuer: 'https://github.com',
            account: 'fixture-login',
            nickname: 'Fixture Nickname',
            avatar_url: 'https://avatars.githubusercontent.com/u/12345',
            profile_url: 'https://github.com/fixture-login',
            bound_at: '2026-10-20T01:00:00Z',
            unlinked_at: null,
            provider_available: true,
            can_unlink: false,
            unlink_blocked_reason: 'last_usable_authentication_method'
          }
        ]
      }
    ]
  };
}
function renderPanel(csrfToken: string | null = 'fixture-csrf') {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  return render(
    <QueryClientProvider client={client}>
      <AuthenticationMethodsPanel
        csrfToken={csrfToken}
        changingPassword={false}
        changePasswordError={null}
        onChangePassword={mocks.change}
        onSwitchAccount={mocks.switchAccount}
      />
    </QueryClientProvider>
  );
}
async function openGitHub() {
  fireEvent.click(await screen.findByRole('tab', { name: 'GitHub' }));
}

// jsdom/nwsapi cannot parse its synthesized :focus-visible selector for
// rc-tabs IDs containing the real slash-bearing provider auth_type. These
// functional fixtures do not assert layout; actual Chrome covers rendering.
const browserComputedStyle = window.getComputedStyle;
beforeEach(() => {
  Object.values(mocks).forEach((mock) => mock.mockReset());
  window.getComputedStyle = (element, pseudoElement) => {
    try {
      return browserComputedStyle(element, pseudoElement);
    } catch (error) {
      const selectorError = error as { name?: unknown; message?: unknown };
      if (
        selectorError.name === 'SyntaxError' &&
        typeof selectorError.message === 'string' &&
        selectorError.message.includes('rc-tabs-test-') &&
        selectorError.message.includes('1flowbase,,github-login') &&
        selectorError.message.includes(':focus-visible') &&
        element instanceof HTMLElement
      ) {
        return element.style;
      }
      throw error;
    }
  };
  window.history.replaceState({}, '', '/me/security');
  mocks.fetch.mockResolvedValue(fixture());
  mocks.authorize.mockRejectedValue(new Error('fixture rejected'));
});
afterEach(() => {
  window.getComputedStyle = browserComputedStyle;
  window.history.replaceState({}, '', '/');
});

test('password is the default and multiple entries produce one provider tab with real binding details', async () => {
  renderPanel();
  expect(
    await screen.findByRole('tab', { name: 'authentication.password_tab' })
  ).toHaveAttribute('aria-selected', 'true');
  await waitFor(() => expect(screen.getAllByRole('tab')).toHaveLength(2));
  await openGitHub();
  expect(screen.getByText('authentication.bound')).toBeVisible();
  expect(screen.getByText('fixture-login')).toBeVisible();
  expect(screen.getAllByText('Fixture Nickname').length).toBeGreaterThan(0);
  expect(
    screen.getByRole('link', { name: 'https://github.com/fixture-login' })
  ).toHaveAttribute('href', 'https://github.com/fixture-login');
  expect(screen.getByRole('img')).toHaveAttribute(
    'src',
    'https://avatars.githubusercontent.com/u/12345'
  );
  expect(screen.getByText('2026-10-20T01:00:00Z')).toBeVisible();
  expect(screen.getByText('authentication.last_login_method')).toBeVisible();
  expect(
    screen.getByRole('button', { name: 'authentication.unlink' })
  ).toBeDisabled();
  expect(
    screen.queryByRole('button', { name: /authentication.bind / })
  ).not.toBeInTheDocument();
  expect(mocks.unlink).not.toHaveBeenCalled();
});

test('unavailable providers retain linked details and the backend still owns unlink eligibility', async () => {
  const methods = fixture();
  methods.providers[0].provider_available = false;
  methods.providers[0].login_entries = [];
  methods.providers[0].bindings[0].provider_available = false;
  methods.providers[0].bindings[0].can_unlink = true;
  methods.providers[0].bindings[0].unlink_blocked_reason = null;
  mocks.fetch.mockResolvedValue(methods);
  renderPanel();
  await openGitHub();
  expect(screen.getByText('authentication.provider_unavailable')).toBeVisible();
  expect(screen.getByText('fixture-login')).toBeVisible();
  expect(
    screen.getByRole('button', { name: 'authentication.unlink' })
  ).toBeEnabled();
});

test('unlink requires confirmation, uses the identity and CSRF, and refreshes persisted state', async () => {
  const methods = fixture();
  methods.providers[0].bindings[0].can_unlink = true;
  methods.providers[0].bindings[0].unlink_blocked_reason = null;
  const unlinked = fixture();
  unlinked.providers[0].bindings[0].state = 'unlinked';
  unlinked.providers[0].bindings[0].unlinked_at = '2026-10-20T02:00:00Z';
  mocks.fetch.mockResolvedValueOnce(methods).mockResolvedValue(unlinked);
  mocks.unlink.mockResolvedValue(undefined);
  renderPanel();
  await openGitHub();
  fireEvent.click(
    screen.getByRole('button', { name: 'authentication.unlink' })
  );
  await waitFor(() =>
    expect(screen.getByText('authentication.unlink_confirm')).toBeVisible()
  );
  expect(mocks.unlink).not.toHaveBeenCalled();
  const buttons = screen.getAllByRole('button', {
    name: 'authentication.unlink'
  });
  fireEvent.click(buttons[buttons.length - 1]);
  await waitFor(() =>
    expect(mocks.unlink).toHaveBeenCalledWith('github-identity', 'fixture-csrf')
  );
  expect(await screen.findByText('authentication.unlinked')).toBeVisible();
  expect(screen.getByText('2026-10-20T02:00:00Z')).toBeVisible();
});

test('unlinked identities offer same-account OAuth restoration and never a fresh bind button', async () => {
  const methods = fixture();
  methods.providers[0].bindings[0].state = 'unlinked';
  mocks.fetch.mockResolvedValue(methods);
  renderPanel();
  await openGitHub();
  expect(screen.getByText('authentication.restore_description')).toBeVisible();
  fireEvent.click(
    screen.getByRole('button', {
      name: 'authentication.restore GitHub production'
    })
  );
  await waitFor(() =>
    expect(mocks.authorize).toHaveBeenCalledWith('github-entry', 'fixture-csrf')
  );
  expect(
    screen.queryByRole('button', { name: /authentication.bind / })
  ).not.toBeInTheDocument();
});

test('an unbound provider allows an explicit bind and no session disables authorization', async () => {
  const methods = fixture();
  methods.providers[0].bindings = [];
  mocks.fetch.mockResolvedValue(methods);
  renderPanel(null);
  await openGitHub();
  expect(screen.getByText('authentication.not_bound')).toBeVisible();
  expect(
    screen.getByRole('button', {
      name: 'authentication.bind GitHub production'
    })
  ).toBeDisabled();
  expect(mocks.authorize).not.toHaveBeenCalled();
});

test.each([false, true])(
  'password form stays local regardless of external setup proof (%s)',
  async (verified) => {
    const methods = fixture();
    methods.password_setup_verified = verified;
    mocks.fetch.mockResolvedValue(methods);
    window.history.replaceState(
      {},
      '',
      '/me/security?auth_verified=true&security_tab=password'
    );
    renderPanel();
    expect(await screen.findByLabelText('auto.old_password')).toBeVisible();
    expect(screen.getByLabelText('auto.new_password')).toBeVisible();
    expect(screen.getByLabelText('auto.confirm_new_password')).toBeVisible();
    expect(
      screen.queryByRole('button', { name: /authentication.verify_with/ })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'authentication.set_password' })
    ).not.toBeInTheDocument();
  }
);

test('built-in password form is available while external discovery is pending', () => {
  mocks.fetch.mockReturnValue(new Promise(() => {}));
  renderPanel();
  expect(screen.getByLabelText('auto.old_password')).toBeVisible();
  expect(
    screen.getByRole('button', { name: 'auto.update_password' })
  ).toBeEnabled();
  expect(screen.queryByRole('tab', { name: 'GitHub' })).not.toBeInTheDocument();
});

test('password change requires the old password and matching confirmation, then submits only old and new passwords', async () => {
  mocks.change.mockResolvedValue(undefined);
  renderPanel();
  const oldPassword = await screen.findByLabelText('auto.old_password');
  const newPassword = screen.getByLabelText('auto.new_password');
  const confirmation = screen.getByLabelText('auto.confirm_new_password');
  const submit = screen.getByRole('button', { name: 'auto.update_password' });
  fireEvent.change(newPassword, { target: { value: 'fixture-new-password' } });
  fireEvent.change(confirmation, { target: { value: 'fixture-new-password' } });
  fireEvent.click(submit);
  expect(
    await screen.findByText('auto.current_password_required')
  ).toBeVisible();
  expect(mocks.change).not.toHaveBeenCalled();
  fireEvent.change(oldPassword, { target: { value: 'fixture-old-password' } });
  fireEvent.change(confirmation, { target: { value: 'fixture-mismatch' } });
  fireEvent.click(submit);
  expect(await screen.findByText('auto.password_mismatch')).toBeVisible();
  expect(mocks.change).not.toHaveBeenCalled();
  fireEvent.change(confirmation, { target: { value: 'fixture-new-password' } });
  fireEvent.click(submit);
  await waitFor(() =>
    expect(mocks.change).toHaveBeenCalledExactlyOnceWith({
      old_password: 'fixture-old-password',
      new_password: 'fixture-new-password'
    })
  );
  expect(mocks.authorize).not.toHaveBeenCalled();
});

test('password changes cannot be submitted without a console session', () => {
  renderPanel(null);
  expect(screen.getByLabelText('auto.old_password')).toBeDisabled();
  expect(
    screen.getByRole('button', { name: 'auto.update_password' })
  ).toBeDisabled();
});

test('foreign-owner conflict offers explicit account switch and cancellation, with no migration action', async () => {
  window.history.replaceState(
    {},
    '',
    '/me/security?auth_error=identity_exists'
  );
  renderPanel();
  expect(
    await screen.findByText('authentication.identity_exists')
  ).toBeVisible();
  expect(
    screen.getByText('authentication.migration_description')
  ).toBeVisible();
  expect(mocks.switchAccount).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole('button', { name: 'authentication.switch_account' })
  );
  await waitFor(() => expect(mocks.switchAccount).toHaveBeenCalledOnce());
  fireEvent.click(
    screen.getByRole('button', { name: 'profile.actions.cancel' })
  );
  expect(
    screen.queryByText('authentication.identity_exists')
  ).not.toBeInTheDocument();
  expect(window.location.search).toBe('');
});

test('external discovery failure preserves the built-in password form without fabricated provider tabs', async () => {
  mocks.fetch.mockRejectedValue(new Error('fixture offline'));
  renderPanel();
  expect(await screen.findByText('authentication.load_failed')).toBeVisible();
  expect(
    screen.getByRole('tab', { name: 'authentication.password_tab' })
  ).toBeVisible();
  expect(screen.getByLabelText('auto.old_password')).toBeVisible();
  expect(screen.queryByRole('tab', { name: 'GitHub' })).not.toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: 'authentication.retry' })
  ).toBeEnabled();
});
