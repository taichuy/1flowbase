import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { StrictMode } from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { apiFetch } from '@1flowbase/api-client/auth';
import { sourceSha256 } from 'virtual:1flowbase-default-password';
import { getAuthApiBaseUrl, type PublicLoginEntry } from '../../../api/session';
import { DefaultPasswordSignIn } from '../DefaultPasswordSignIn';
import { matchesDefaultPasswordSource } from '../source-identity';

vi.mock('@1flowbase/api-client/auth', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client/auth')>()),
  apiFetch: vi.fn()
}));

afterEach(() => {
  // Reset our request double, not shared DOM shims from the standard setup.
  vi.mocked(apiFetch).mockReset();
  vi.clearAllMocks();
});

const source = readFileSync(
  path.resolve(
    process.cwd(),
    '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
  ),
  'utf8'
);
const session = {
  csrf_token: 'csrf',
  effective_display_role: 'member',
  current_workspace_id: 'workspace'
};
function entry(
  publicVariables: Record<string, unknown> = {},
  id = 'password-local'
): PublicLoginEntry {
  return {
    id,
    auth_type: 'password-local',
    entry_action: 'render_ui',
    title: 'Password',
    sort_order: 0,
    is_builtin: true,
    public_ui_block: source,
    public_variables: publicVariables
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function fillSignIn() {
  fireEvent.change(screen.getByLabelText('Account or email'), {
    target: { value: 'alice' }
  });
  fireEvent.change(screen.getByLabelText('Password'), {
    target: { value: 'secret' }
  });
}
function submit() {
  fireEvent.submit(screen.getByLabelText('Password').closest('form')!);
}

test('exact UTF-8 SHA identifies the backend source; whitespace and same-length modifications fail closed', () => {
  expect(sourceSha256).toBe(createHash('sha256').update(source).digest('hex'));
  expect(matchesDefaultPasswordSource(source)).toBe(true);
  expect(matchesDefaultPasswordSource(`${source}\n`)).toBe(false);
  expect(
    matchesDefaultPasswordSource(source.replace('Sign in', 'Sign In'))
  ).toBe(false);
  expect(matchesDefaultPasswordSource(source.replaceAll('\n', '\r\n'))).toBe(
    false
  );
  expect(matchesDefaultPasswordSource('')).toBe(false);
});

test('a mismatched source never renders the official form (the caller owns legacy fallback)', () => {
  render(
    <DefaultPasswordSignIn
      instance={{ ...entry(), public_ui_block: source + '\n' }}
      onAuthenticated={vi.fn()}
    />
  );
  expect(
    screen.queryByRole('button', { name: 'Sign in' })
  ).not.toBeInTheDocument();
});

test('uses native form controls and the existing POST client; pending lasts through the authentication callback', async () => {
  const response = deferred<unknown>();
  const authenticated = deferred<void>();
  vi.mocked(apiFetch).mockReturnValue(response.promise);
  const onAuthenticated = vi.fn(() => authenticated.promise);
  const request = vi.fn();
  render(
    <StrictMode>
      <DefaultPasswordSignIn
        instance={entry({ self_registration_enabled: true })}
        loginEntrySelector={{ request }}
        onAuthenticated={onAuthenticated}
      />
    </StrictMode>
  );
  fillSignIn();
  // Exercise the real plugin/TSX path, not an Antd-backed lookalike.
  expect(screen.getByLabelText('Password')).toHaveClass('auth-password-input');
  expect(screen.getByRole('button', { name: 'Sign in' })).toHaveClass(
    'auth-password-button-primary'
  );
  expect(document.querySelector('.ant-input, .ant-btn, .ant-space')).toBeNull();
  expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password');
  expect(screen.getByLabelText('Password')).toHaveAttribute(
    'autocomplete',
    'current-password'
  );
  expect(screen.getByLabelText('Password')).toBeRequired();
  // The backend uses Input(type=password), NOT Input.Password: no invented reveal control.
  expect(
    screen.queryByRole('button', { name: /show password|hide password/i })
  ).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Sign in' })).toHaveAttribute(
    'type',
    'submit'
  );
  submit(); // Native submit is also the keyboard/Enter path; real keyboard coverage belongs to Root's browser batch.
  expect(apiFetch).toHaveBeenCalledWith({
    method: 'POST',
    path: '/api/public/auth/sign-in',
    body: {
      login_entry_id: 'password-local',
      identifier: 'alice',
      password: 'secret'
    },
    baseUrl: getAuthApiBaseUrl()
  });
  expect(screen.getByRole('button', { name: 'Sign in' })).toHaveAttribute(
    'aria-busy',
    'true'
  );
  expect(
    screen.getByRole('button', { name: 'Create an account' })
  ).toBeDisabled();
  expect(
    screen.getByRole('button', { name: 'Back to other sign-in options' })
  ).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
  expect(apiFetch).toHaveBeenCalledTimes(1);
  expect(onAuthenticated).not.toHaveBeenCalled();
  await act(async () => {
    response.resolve(session);
  });
  expect(onAuthenticated).toHaveBeenCalledWith(session);
  expect(screen.getByRole('button', { name: 'Sign in' })).toHaveAttribute(
    'aria-busy',
    'true'
  );
  await act(async () => {
    authenticated.resolve();
  });
  expect(screen.getByRole('button', { name: 'Sign in' })).not.toHaveAttribute(
    'aria-busy',
    'true'
  );
});

test.each([undefined, false, 'true', 1])(
  'registration is not enabled by %s',
  (value) => {
    render(
      <DefaultPasswordSignIn
        instance={entry({ self_registration_enabled: value })}
        onAuthenticated={vi.fn()}
      />
    );
    expect(
      screen.queryByRole('button', { name: 'Create an account' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Back to other sign-in options' })
    ).not.toBeInTheDocument();
  }
);

test('strict registration true preserves sign-up fields, optional email, password state and completion', async () => {
  vi.mocked(apiFetch).mockResolvedValue(session);
  const onAuthenticated = vi.fn();
  render(
    <DefaultPasswordSignIn
      instance={entry({ self_registration_enabled: true })}
      onAuthenticated={onAuthenticated}
    />
  );
  fillSignIn();
  fireEvent.click(screen.getByRole('button', { name: 'Create an account' }));
  expect(
    screen.getByRole('heading', { name: 'Create an account' })
  ).toBeInTheDocument();
  expect(screen.getByLabelText('Email')).not.toBeRequired();
  expect(screen.getByLabelText('Email')).toHaveAttribute('type', 'email');
  expect(screen.getByLabelText('Password')).toHaveAttribute(
    'autocomplete',
    'new-password'
  );
  expect(screen.getByLabelText('Password')).toHaveValue('secret');
  fireEvent.change(screen.getByLabelText('Account'), {
    target: { value: 'new-account' }
  });
  fireEvent.change(screen.getByLabelText('Email'), {
    target: { value: 'new@example.com' }
  });
  submit();
  expect(apiFetch).toHaveBeenCalledWith(
    expect.objectContaining({
      path: '/api/public/auth/sign-up',
      body: {
        login_entry_id: 'password-local',
        account: 'new-account',
        email: 'new@example.com',
        password: 'secret'
      }
    })
  );
  await waitFor(() => expect(onAuthenticated).toHaveBeenCalledWith(session));
  await waitFor(() =>
    expect(
      screen.getByRole('button', { name: 'Back to sign in' })
    ).toBeEnabled()
  );
  fireEvent.click(screen.getByRole('button', { name: 'Back to sign in' }));
  expect(screen.getByLabelText('Account or email')).toHaveValue('alice');
});

test('the selector button emits only its existing request callback', () => {
  const request = vi.fn();
  render(
    <DefaultPasswordSignIn
      instance={entry()}
      loginEntrySelector={{ request }}
      onAuthenticated={vi.fn()}
    />
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Back to other sign-in options' })
  );
  expect(request).toHaveBeenCalledTimes(1);
  expect(apiFetch).not.toHaveBeenCalled();
});

test.each([
  ['sign-in', new Error('Wrong credentials'), 'Wrong credentials'],
  ['sign-in', 'not an Error', 'Authentication failed'],
  ['sign-up', new Error('Account exists'), 'Account exists'],
  ['sign-up', null, 'Registration failed']
])(
  '%s displays source-defined error copy and recovers on retry',
  async (mode, error, message) => {
    vi.mocked(apiFetch)
      .mockRejectedValueOnce(error)
      .mockResolvedValueOnce(session);
    const onAuthenticated = vi.fn();
    render(
      <DefaultPasswordSignIn
        instance={entry({ self_registration_enabled: true })}
        onAuthenticated={onAuthenticated}
      />
    );
    if (mode === 'sign-up')
      fireEvent.click(
        screen.getByRole('button', { name: 'Create an account' })
      );
    submit();
    expect(await screen.findByRole('alert')).toHaveTextContent(message);
    expect(onAuthenticated).not.toHaveBeenCalled();
    submit();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    await waitFor(() => expect(onAuthenticated).toHaveBeenCalledWith(session));
  }
);

test.each([
  null,
  [],
  { csrf_token: 'partial' },
  { ...session, current_workspace_id: null }
])('does not authenticate a non-canonical response: %j', async (response) => {
  vi.mocked(apiFetch).mockResolvedValue(response);
  const onAuthenticated = vi.fn();
  render(
    <DefaultPasswordSignIn
      instance={entry()}
      onAuthenticated={onAuthenticated}
    />
  );
  submit();
  await waitFor(() =>
    expect(screen.getByRole('button', { name: 'Sign in' })).not.toHaveAttribute(
      'aria-busy',
      'true'
    )
  );
  expect(onAuthenticated).not.toHaveBeenCalled();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

test('callback rejection is awaited and handled by the original template error branch', async () => {
  vi.mocked(apiFetch).mockResolvedValue(session);
  const onAuthenticated = vi
    .fn()
    .mockRejectedValue(new Error('Session handoff failed'));
  render(
    <DefaultPasswordSignIn
      instance={entry()}
      onAuthenticated={onAuthenticated}
    />
  );
  submit();
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'Session handoff failed'
  );
  expect(screen.getByRole('button', { name: 'Sign in' })).not.toHaveAttribute(
    'aria-busy',
    'true'
  );
});

test.each(['different-id', 'same-id', 'unmount'])(
  'a late %s response uses only the captured callback and never the replacement form/callback',
  async (change) => {
    const response = deferred<unknown>();
    vi.mocked(apiFetch)
      .mockReturnValueOnce(response.promise)
      .mockResolvedValue(session);
    const onAuthenticated = vi.fn();
    const nextAuthenticated = vi.fn();
    const { unmount, rerender } = render(
      <DefaultPasswordSignIn
        instance={entry()}
        onAuthenticated={onAuthenticated}
      />
    );
    fillSignIn();
    submit();
    if (change === 'unmount') unmount();
    else
      rerender(
        <DefaultPasswordSignIn
          instance={entry(
            {},
            change === 'same-id' ? 'password-local' : 'next-entry'
          )}
          onAuthenticated={nextAuthenticated}
        />
      );
    await act(async () => {
      response.resolve(session);
    });
    expect(onAuthenticated).toHaveBeenCalledWith(session);
    expect(nextAuthenticated).not.toHaveBeenCalled();
    if (change !== 'unmount') {
      expect(screen.getByLabelText('Account or email')).toHaveValue('');
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      fillSignIn();
      submit();
      await waitFor(() => expect(nextAuthenticated).toHaveBeenCalledTimes(1));
      expect(onAuthenticated).toHaveBeenCalledTimes(1);
    }
  }
);

test('stable capability owners preserve local state; replacing only the callback starts a fresh instance', async () => {
  const instance = entry();
  const response = deferred<unknown>();
  vi.mocked(apiFetch).mockReturnValue(response.promise);
  const originalCallback = vi.fn();
  const replacementCallback = vi.fn();
  const { rerender } = render(
    <DefaultPasswordSignIn
      instance={instance}
      onAuthenticated={originalCallback}
    />
  );
  fillSignIn();
  rerender(
    <DefaultPasswordSignIn
      instance={instance}
      onAuthenticated={originalCallback}
    />
  );
  expect(screen.getByLabelText('Account or email')).toHaveValue('alice');
  submit();
  rerender(
    <DefaultPasswordSignIn
      instance={instance}
      onAuthenticated={replacementCallback}
    />
  );
  expect(screen.getByLabelText('Account or email')).toHaveValue('');
  await act(async () => {
    response.resolve(session);
  });
  expect(originalCallback).toHaveBeenCalledWith(session);
  expect(replacementCallback).not.toHaveBeenCalled();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

test('attaches invitation_token to sign-in and sign-up without changing official password source', async () => {
  vi.mocked(apiFetch).mockResolvedValue(session);
  const onAuthenticated = vi.fn();
  render(
    <DefaultPasswordSignIn
      instance={entry({ self_registration_enabled: true })}
      invitationToken="inv_alpha_secret"
      onAuthenticated={onAuthenticated}
    />
  );

  fillSignIn();
  submit();
  expect(apiFetch).toHaveBeenCalledWith(
    expect.objectContaining({
      path: '/api/public/auth/sign-in',
      body: {
        login_entry_id: 'password-local',
        identifier: 'alice',
        password: 'secret',
        invitation_token: 'inv_alpha_secret'
      }
    })
  );
  await waitFor(() => expect(onAuthenticated).toHaveBeenCalledTimes(1));

  fireEvent.click(screen.getByRole('button', { name: 'Create an account' }));
  fireEvent.change(screen.getByLabelText('Account'), {
    target: { value: 'invited-user' }
  });
  fireEvent.change(screen.getByLabelText('Email'), {
    target: { value: 'invited@example.com' }
  });
  submit();
  expect(apiFetch).toHaveBeenLastCalledWith(
    expect.objectContaining({
      path: '/api/public/auth/sign-up',
      body: {
        login_entry_id: 'password-local',
        account: 'invited-user',
        email: 'invited@example.com',
        password: 'secret',
        invitation_token: 'inv_alpha_secret'
      }
    })
  );
  await waitFor(() => expect(onAuthenticated).toHaveBeenCalledTimes(2));
});
