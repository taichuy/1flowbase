import { afterEach, expect, test, vi } from 'vitest';

import {
  fetchConsoleSession,
  redeemConsoleWorkspaceInvitation,
  switchConsoleSessionRole,
  switchConsoleSessionWorkspace,
  type ConsoleSessionSnapshot
} from '../../../console/session';

afterEach(() => vi.restoreAllMocks());

test('session reads and transitions preserve the policy fingerprint DTO', async () => {
  const snapshot = {
    actor: {
      id: 'user',
      account: 'member',
      effective_display_role: 'reader',
      current_workspace_id: 'workspace'
    },
    session: {
      id: 'session',
      user_id: 'user',
      tenant_id: 'tenant',
      current_workspace_id: 'workspace',
      active_role_code: 'reader'
    },
    available_roles: [{ code: 'reader', name: 'Reader', scope_kind: 'workspace' }],
    policy_fingerprint: '000102030405060708090a0b0c0d0e0f',
    csrf_token: 'csrf',
    cookie_name: 'session'
  } satisfies ConsoleSessionSnapshot;
  const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
    Response.json({ data: snapshot })
  );
  const base = 'http://127.0.0.1:7800';
  await expect(fetchConsoleSession(base)).resolves.toEqual(snapshot);
  await expect(switchConsoleSessionRole('reader', 'csrf', base)).resolves.toEqual(snapshot);
  await expect(switchConsoleSessionWorkspace('workspace', 'csrf', base)).resolves.toEqual(snapshot);
  await expect(redeemConsoleWorkspaceInvitation('fixture', 'csrf', base)).resolves.toEqual(snapshot);
  expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
    `${base}/api/console/session`,
    `${base}/api/console/session/actions/switch-role`,
    `${base}/api/console/session/actions/switch-workspace`,
    `${base}/api/console/workspaces/invitations/redeem`
  ]);
});
