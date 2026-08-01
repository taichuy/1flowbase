const test = require('node:test');
const assert = require('node:assert/strict');

const {
  loadRootCredentials,
  openTemporaryConsoleSession,
  openTemporaryOwnerSession,
} = require('../auth.js');

test('loadRootCredentials falls back to api-server bootstrap env values', () => {
  const credentials = loadRootCredentials({
    repoRoot: '/repo',
    accountOverride: null,
    passwordOverride: null,
    getServiceDefinitions: () => ({
      'api-server': { key: 'api-server', envFile: '/repo/api/apps/api-server/.env' },
    }),
    buildServiceEnv: () => ({
      BOOTSTRAP_ROOT_ACCOUNT: 'root',
      BOOTSTRAP_ROOT_PASSWORD: 'change-me',
    }),
  });

  assert.deepEqual(credentials, {
    account: 'root',
    password: 'change-me',
    envFilePath: '/repo/api/apps/api-server/.env',
  });
});

test('AC-001 openTemporaryConsoleSession persists auth state and revokes only its own session on dispose', async () => {
  const calls = [];
  const fakeRequestContext = {
    post: async (path, options) => {
      calls.push({ path, options });
      return {
        ok: () => true,
        status: () => 200,
        json: async () => ({ data: { csrf_token: 'csrf-token' } }),
      };
    },
    storageState: async ({ path }) => {
      calls.push({ storageStatePath: path });
    },
    delete: async (path, options) => {
      calls.push({ deletePath: path, options });
      return {
        ok: () => true,
        status: () => 204,
      };
    },
    dispose: async () => {
      calls.push({ dispose: true });
    },
  };

  const result = await openTemporaryConsoleSession({
    playwright: {
      request: {
        newContext: async () => fakeRequestContext,
      },
    },
    apiBaseUrl: 'http://127.0.0.1:7800',
    account: 'root',
    password: 'change-me',
    storageStatePath: '/tmp/page-debug/storage-state.json',
  });

  assert.equal(result.authenticated, true);
  assert.equal(result.storageStatePath, '/tmp/page-debug/storage-state.json');
  assert.equal(calls.some((call) => call.deletePath), false);
  assert.deepEqual(calls[0], {
    path: '/api/public/auth/sign-in',
    options: {
      data: {
        authenticator_id: '00000000-0000-0000-0000-000000000001',
        identifier: 'root',
        password: 'change-me',
      },
    },
  });
  await result.dispose();
  assert.deepEqual(calls.at(-2), {
    deletePath: '/api/console/session',
    options: {
      headers: {
        'x-csrf-token': 'csrf-token',
      },
    },
  });
  assert.deepEqual(calls.at(-1), { dispose: true });
});

test('openTemporaryConsoleSession surfaces not_authenticated guidance on 401', async () => {
  await assert.rejects(
    () =>
      openTemporaryConsoleSession({
        playwright: {
          request: {
            newContext: async () => ({
              post: async () => ({
                ok: () => false,
                status: () => 401,
                text: async () => 'not_authenticated',
              }),
              dispose: async () => {},
            }),
          },
        },
        apiBaseUrl: 'http://127.0.0.1:7800',
        account: 'root',
        password: 'wrong',
        storageStatePath: '/tmp/page-debug/storage-state.json',
      }),
    /root 凭据无效|not_authenticated/u
  );
});

test('AC-002 openTemporaryConsoleSession revokes the issued session when storage export fails', async () => {
  let deleteCalled = false;
  let requestContextDisposed = false;

  await assert.rejects(
    () =>
      openTemporaryConsoleSession({
        playwright: {
          request: {
            newContext: async () => ({
              post: async () => ({
                ok: () => true,
                status: () => 200,
                json: async () => ({ data: { csrf_token: 'csrf-token' } }),
              }),
              storageState: async () => {
                throw new Error('storage export failed');
              },
              delete: async () => {
                deleteCalled = true;
                return { ok: () => true, status: () => 204 };
              },
              dispose: async () => {
                requestContextDisposed = true;
              },
            }),
          },
        },
        apiBaseUrl: 'http://127.0.0.1:7800',
        account: 'root',
        password: 'change-me',
        storageStatePath: '/tmp/page-debug/storage-state.json',
      }),
    /storage export failed/u
  );

  assert.equal(deleteCalled, true);
  assert.equal(requestContextDisposed, true);
});

test('AC-003 openTemporaryConsoleSession skips storage export for credentials-only mode and still revokes', async () => {
  let storageStateCalled = false;
  let deleteCalled = false;

  const result = await openTemporaryConsoleSession({
    playwright: {
      request: {
        newContext: async () => ({
          post: async () => ({
            ok: () => true,
            status: () => 200,
            json: async () => ({ data: { csrf_token: 'csrf-token' } }),
          }),
          storageState: async () => {
            storageStateCalled = true;
          },
          delete: async () => {
            deleteCalled = true;
            return { ok: () => true, status: () => 204 };
          },
          dispose: async () => {},
        }),
      },
    },
    apiBaseUrl: 'http://127.0.0.1:7800',
    account: 'root',
    password: 'change-me',
    storageStatePath: null,
  });

  assert.equal(result.authenticated, true);
  assert.equal(result.storageStatePath, null);
  assert.equal(storageStateCalled, false);
  await result.dispose();
  assert.equal(deleteCalled, true);
});

test('AC-004 independently disposes concurrent temporary console sessions', async () => {
  const deleted = [];
  let nextContext = 0;
  const contexts = ['session-a', 'session-b'].map((sessionId) => ({
    post: async () => ({
      ok: () => true,
      status: () => 200,
      json: async () => ({ data: { csrf_token: `csrf-${sessionId}` } }),
    }),
    storageState: async () => {},
    delete: async (_path, options) => {
      deleted.push({ sessionId, csrfToken: options.headers['x-csrf-token'] });
      return { ok: () => true, status: () => 204 };
    },
    dispose: async () => {},
  }));

  const playwright = {
    request: {
      newContext: async () => contexts[nextContext++],
    },
  };
  const input = {
    playwright,
    apiBaseUrl: 'http://127.0.0.1:7800',
    account: 'root',
    password: 'change-me',
    storageStatePath: null,
  };
  const [first, second] = await Promise.all([
    openTemporaryConsoleSession(input),
    openTemporaryConsoleSession(input),
  ]);

  await first.dispose();
  assert.deepEqual(deleted, [{ sessionId: 'session-a', csrfToken: 'csrf-session-a' }]);
  await second.dispose();
  assert.deepEqual(deleted, [
    { sessionId: 'session-a', csrfToken: 'csrf-session-a' },
    { sessionId: 'session-b', csrfToken: 'csrf-session-b' },
  ]);
});

test('Root #1556 F11 opens and revokes a fetch-based owner session on the owned API', async () => {
  const calls = [];
  const session = await openTemporaryOwnerSession({
    apiBaseUrl: 'http://127.0.0.1:41732',
    account: 'root',
    password: 'owner-password',
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      if (init.method === 'POST') {
        return new Response(JSON.stringify({ data: { csrf_token: 'owned-csrf' } }), {
          status: 200,
          headers: { 'set-cookie': 'flowbase_console_session=owned-cookie; HttpOnly; Path=/' },
        });
      }
      return new Response(null, { status: 204 });
    },
  });

  assert.equal(session.cookie, 'flowbase_console_session=owned-cookie');
  assert.equal(session.csrfToken, 'owned-csrf');
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    authenticator_id: '00000000-0000-0000-0000-000000000001',
    identifier: 'root',
    password: 'owner-password',
  });
  await session.dispose();
  assert.equal(calls[1].url, 'http://127.0.0.1:41732/api/console/session');
  assert.deepEqual(calls[1].init.headers, {
    cookie: 'flowbase_console_session=owned-cookie',
    'x-csrf-token': 'owned-csrf',
  });
});

test('Root #1556 F11 owner session preserves login and revoke failures', async () => {
  await assert.rejects(
    () => openTemporaryOwnerSession({
      apiBaseUrl: 'http://127.0.0.1:41732',
      account: 'root',
      password: 'wrong',
      fetchImpl: async () => new Response('not_authenticated', { status: 401 }),
    }),
    /not_authenticated/u,
  );

  const session = await openTemporaryOwnerSession({
    apiBaseUrl: 'http://127.0.0.1:41732',
    account: 'root',
    password: 'owner-password',
    fetchImpl: async (_url, init) => init.method === 'POST'
      ? new Response(JSON.stringify({ data: { csrf_token: 'owned-csrf' } }), {
        status: 200,
        headers: { 'set-cookie': 'flowbase_console_session=owned-cookie; Path=/' },
      })
      : new Response('revoke fixture', { status: 500 }),
  });
  await assert.rejects(() => session.dispose(), /revoke fixture/u);
});
