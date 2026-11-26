import { apiFetch } from '@1flowbase/api-client/auth';
import { afterEach, expect, test, vi } from 'vitest';
import { getAuthApiBaseUrl } from '../../../api/session';
import {
  createDefaultPasswordContext,
  type DefaultPasswordSignInProps
} from '../context';

vi.mock('@1flowbase/api-client/auth', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client/auth')>()),
  apiFetch: vi.fn()
}));
afterEach(() => vi.resetAllMocks());
const session = {
  csrf_token: 'csrf',
  effective_display_role: 'member',
  current_workspace_id: 'workspace'
};
function fixture() {
  const onAuthenticated = vi.fn();
  const request = vi.fn();
  const props: DefaultPasswordSignInProps = {
    instance: {
      id: 'password-local',
      entry_action: 'render_ui',
      auth_type: 'password-local',
      title: 'Password',
      sort_order: 0,
      is_builtin: true,
      public_ui_block: '',
      public_variables: { self_registration_enabled: true }
    },
    loginEntrySelector: { request },
    onAuthenticated
  };
  return { props, onAuthenticated, request };
}

test('revoked capabilities reject BEFORE dispatch or selector effects', async () => {
  const { props, request, onAuthenticated } = fixture();
  const ctx = createDefaultPasswordContext(props, () => false);
  await expect(ctx.api.post('/api/public/auth/sign-in')).rejects.toThrow(
    'Native Block capability belongs to a stale instance.'
  );
  expect(() => ctx.events.emit('authenticator_selector_requested')).toThrow(
    'Native Block capability belongs to a stale instance.'
  );
  expect(apiFetch).not.toHaveBeenCalled();
  expect(request).not.toHaveBeenCalled();
  expect(onAuthenticated).not.toHaveBeenCalled();
});

test.each([
  'https://elsewhere.example/api/public/auth/sign-in',
  '/api/console/session',
  '/api/public/../console/session'
])('rejects forbidden public-auth paths: %s', async (path) => {
  const { props } = fixture();
  const ctx = createDefaultPasswordContext(props, () => true);
  await expect(ctx.api.post(path)).rejects.toThrow(
    'Public authentication Block requested a forbidden API path.'
  );
  expect(apiFetch).not.toHaveBeenCalled();
});

test('post-response stale checking follows the captured callback, including callback navigation/unmount', async () => {
  const { props, onAuthenticated } = fixture();
  let current = true;
  onAuthenticated.mockImplementation(() => {
    current = false;
  });
  vi.mocked(apiFetch).mockResolvedValue(session);
  const ctx = createDefaultPasswordContext(props, () => current);
  await expect(ctx.api.post('/api/public/auth/sign-in')).rejects.toThrow(
    'Native Block capability belongs to a stale instance.'
  );
  expect(onAuthenticated).toHaveBeenCalledWith(session);
});

test('only exact completion paths notify auth; other public requests retain path/search and response', async () => {
  const { props, onAuthenticated, request } = fixture();
  vi.mocked(apiFetch).mockResolvedValue(session);
  const ctx = createDefaultPasswordContext(props, () => true);
  await expect(
    ctx.api.post('/api/public/auth/sign-in?extra=1', { body: { value: 1 } })
  ).resolves.toEqual(session);
  expect(apiFetch).toHaveBeenCalledWith({
    method: 'POST',
    path: '/api/public/auth/sign-in?extra=1',
    body: { value: 1 },
    baseUrl: getAuthApiBaseUrl()
  });
  expect(onAuthenticated).not.toHaveBeenCalled();
  ctx.events.emit('unknown_event');
  expect(request).not.toHaveBeenCalled();
  ctx.events.emit('authenticator_selector_requested');
  expect(request).toHaveBeenCalledTimes(1);
  expect(ctx.inputs).toEqual({
    login_entry_id: 'password-local',
    authenticator_selection_available: true,
    public_variables: { self_registration_enabled: true }
  });
});
