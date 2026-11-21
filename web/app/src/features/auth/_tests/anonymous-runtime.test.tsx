import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { StrictMode } from 'react';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import type { PublicAuthBlockProps } from '../components/PublicAuthBlock';

const fixture = vi.hoisted(() => ({
  discover: vi.fn(),
  me: vi.fn(),
  session: vi.fn(),
  custom: vi.fn(),
  customImports: vi.fn(),
  mcpImports: vi.fn(),
  callbacks: [] as PublicAuthBlockProps['onAuthenticated'][]
}));
vi.mock('../api/login-entry-discovery', () => ({
  consumeLoginEntryDiscovery: fixture.discover
}));
vi.mock('../api/session', async () => ({
  ...(await vi.importActual<typeof import('../api/session')>('../api/session')),
  fetchCurrentMe: fixture.me,
  fetchCurrentSession: fixture.session
}));
vi.mock('../lib/public-system-metadata', () => ({
  usePublicSystemMetadata: () => ({ data: { hide_login_tagline: true } })
}));
vi.mock('../components/public-runtime/CustomAuthRuntime', () => {
  fixture.customImports();
  return {
    CustomAuthRuntime: (props: PublicAuthBlockProps) => {
      fixture.custom(props.instance.public_ui_block);
      fixture.callbacks.push(props.onAuthenticated);
      return (
        <form aria-label="Custom form">
          <button
            type="button"
            onClick={() =>
              props.onAuthenticated({
                csrf_token: 'not-canonical',
                effective_display_role: 'member',
                current_workspace_id: 'not-canonical'
              })
            }
          >
            Custom sign in
          </button>
        </form>
      );
    }
  };
});
vi.mock('../components/public-runtime/McpAuthRuntime', () => {
  fixture.mcpImports();
  return {
    McpAuthRuntime: ({ requestId }: { requestId?: string }) => (
      <p>
        MCP request: {requestId}, origin:{' '}
        {new URLSearchParams(location.search).get('origin')}
      </p>
    )
  };
});
import { PublicRouterProvider } from '../../../app/public-router';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';

const source = readFileSync(
  resolve(
    '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
  ),
  'utf8'
);
const password = {
  id: 'password',
  auth_type: 'password-local',
  is_builtin: true,
  entry_action: 'render_ui',
  title: 'Password',
  description: '',
  sort_order: 0,
  public_ui_block: source,
  public_variables: { self_registration_enabled: false }
};
beforeEach(() => {
  Object.values(fixture).forEach((value) => {
    if (vi.isMockFunction(value)) value.mockClear();
  });
  fixture.callbacks.length = 0;
  localStorage.clear();
  history.replaceState({}, '', '/sign-in');
  resetAuthStore();
  useAuthStore.getState().setAnonymous();
  fixture.discover.mockResolvedValue({ login_entries: [password] });
  fixture.me.mockResolvedValue({ id: 'me', account: 'canonical' });
  fixture.session.mockResolvedValue({
    csrf_token: 'canonical-csrf',
    actor: {
      id: 'me',
      account: 'canonical',
      current_workspace_id: 'workspace',
      effective_display_role: 'member'
    },
    available_roles: [{ code: 'member' }]
  });
});

test('exact default source renders the real fast form in StrictMode without custom/MCP runtime imports', async () => {
  render(
    <StrictMode>
      <PublicRouterProvider />
    </StrictMode>
  );
  expect(await screen.findByTestId('default-password-sign-in')).toBeVisible();
  expect(document.querySelector('form')).not.toBeNull();
  expect(fixture.customImports).not.toHaveBeenCalled();
  expect(fixture.mcpImports).not.toHaveBeenCalled();
  expect(fixture.custom).not.toHaveBeenCalled();
});
test('a one-byte mismatch chooses custom runtime and still hydrates canonical me/session/auth store', async () => {
  fixture.discover.mockResolvedValue({
    login_entries: [{ ...password, public_ui_block: source + '\n' }]
  });
  render(<PublicRouterProvider />);
  fireEvent.click(
    await screen.findByRole('button', { name: 'Custom sign in' })
  );
  await waitFor(() =>
    expect(useAuthStore.getState().sessionStatus).toBe('authenticated')
  );
  expect(fixture.custom).toHaveBeenCalledWith(source + '\n');
  expect(fixture.me).toHaveBeenCalledTimes(1);
  expect(fixture.session).toHaveBeenCalledTimes(1);
  expect(useAuthStore.getState().csrfToken).toBe('canonical-csrf');
  expect(useAuthStore.getState().me?.account).toBe('canonical');
  expect(useAuthStore.getState().actor?.current_workspace_id).toBe('workspace');
});
test('entry choice and native Back/Forward preserve language/error without re-discovery', async () => {
  history.replaceState({}, '', '/sign-in?language=en&auth_error=cancelled');
  fixture.discover.mockResolvedValue({
    login_entries: [
      password,
      {
        ...password,
        id: 'custom',
        title: 'Custom',
        public_ui_block: source + '\n'
      }
    ]
  });
  render(<PublicRouterProvider />);
  fireEvent.click(await screen.findByRole('button', { name: 'Password' }));
  expect(await screen.findByTestId('default-password-sign-in')).toBeVisible();
  expect(location.search).toBe(
    '?language=en&auth_error=cancelled&login_entry_id=password'
  );
  expect(screen.getByRole('alert')).toHaveTextContent(
    'GitHub authorization was cancelled.'
  );
  await act(async () => {
    history.replaceState({}, '', '/sign-in?language=en&auth_error=cancelled');
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(await screen.findByRole('button', { name: 'Custom' })).toBeVisible();
  await act(async () => {
    history.replaceState(
      {},
      '',
      '/sign-in?language=en&auth_error=cancelled&login_entry_id=custom'
    );
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(
    await screen.findByRole('button', { name: 'Custom sign in' })
  ).toBeVisible();
  expect(fixture.discover).toHaveBeenCalledTimes(1);
});
test('MCP selection is lazy and keeps request_id/origin through popstate', async () => {
  history.replaceState(
    {},
    '',
    '/mcp/authorize?request_id=one&origin=https%3A%2F%2Fexample.test'
  );
  render(<PublicRouterProvider />);
  expect(
    await screen.findByText('MCP request: one, origin: https://example.test')
  ).toBeVisible();
  expect(fixture.discover).not.toHaveBeenCalled();
  await act(async () => {
    history.replaceState(
      {},
      '',
      '/mcp/authorize?request_id=two&origin=https%3A%2F%2Fother.test'
    );
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(
    await screen.findByText('MCP request: two, origin: https://other.test')
  ).toBeVisible();
});
test('stale custom callbacks and in-flight canonical hydration cannot sign in after route unmount', async () => {
  fixture.discover.mockResolvedValue({
    login_entries: [{ ...password, public_ui_block: 'custom' }]
  });
  let finish!: (value: unknown) => void;
  fixture.me.mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    })
  );
  const view = render(<PublicRouterProvider />);
  fireEvent.click(
    await screen.findByRole('button', { name: 'Custom sign in' })
  );
  const callback = fixture.callbacks.at(-1)!;
  view.unmount();
  await act(async () => {
    finish({ id: 'stale' });
    await callback({
      csrf_token: 'stale',
      effective_display_role: 'member',
      current_workspace_id: 'stale'
    });
  });
  expect(useAuthStore.getState().sessionStatus).toBe('anonymous');
  expect(fixture.me).toHaveBeenCalledTimes(1);
});
