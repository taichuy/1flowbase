const test = require('node:test');
const assert = require('node:assert/strict');

const { loadRootCredentials, loginAndPersistStorageState } = require('../auth.js');

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

test('loginAndPersistStorageState posts password login and writes storageState', async () => {
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
    dispose: async () => {
      calls.push({ dispose: true });
    },
  };

  const result = await loginAndPersistStorageState({
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
});

test('loginAndPersistStorageState surfaces not_authenticated guidance on 401', async () => {
  await assert.rejects(
    () =>
      loginAndPersistStorageState({
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

test('loginAndPersistStorageState skips storage export when storageStatePath is null', async () => {
  let storageStateCalled = false;

  const result = await loginAndPersistStorageState({
    playwright: {
      request: {
        newContext: async () => ({
          post: async () => ({
            ok: () => true,
            status: () => 200,
            json: async () => ({ data: {} }),
          }),
          storageState: async () => {
            storageStateCalled = true;
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
});
