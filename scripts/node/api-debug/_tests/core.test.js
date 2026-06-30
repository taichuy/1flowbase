const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  parseCliArgs,
  redactSensitiveHeaders,
  resolveTargetUrl,
  runApiDebug,
} = require('../core.js');

test('parseCliArgs accepts default GET target and request options', () => {
  const options = parseCliArgs([
    '/api/console/me',
    '--header',
    'x-debug: yes',
    '--expect-status',
    '200',
  ]);

  assert.equal(options.method, 'GET');
  assert.equal(options.target, '/api/console/me');
  assert.deepEqual(options.headers, [{ name: 'x-debug', value: 'yes' }]);
  assert.equal(options.expectStatus, 200);
});

test('parseCliArgs accepts explicit method and body', () => {
  const options = parseCliArgs(['POST', '/api/console/things', '--body', '{"name":"demo"}']);

  assert.equal(options.method, 'POST');
  assert.equal(options.target, '/api/console/things');
  assert.equal(options.body, '{"name":"demo"}');
});

test('resolveTargetUrl resolves API-relative paths against base url', () => {
  assert.equal(
    resolveTargetUrl('http://127.0.0.1:7800', '/api/console/me'),
    'http://127.0.0.1:7800/api/console/me'
  );
});

test('redactSensitiveHeaders removes credentials from evidence', () => {
  const redacted = redactSensitiveHeaders({
    cookie: 'session=secret',
    authorization: 'Bearer token',
    'x-csrf-token': 'csrf-token',
    'content-type': 'application/json',
  });

  assert.equal(redacted.cookie, '[redacted]');
  assert.equal(redacted.authorization, '[redacted]');
  assert.equal(redacted['x-csrf-token'], '[redacted]');
  assert.equal(redacted['content-type'], 'application/json');
});

test('runApiDebug logs in with env credentials and sends authenticated mutating request', async () => {
  const calls = [];
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-api-debug-'));

  const result = await runApiDebug(
    {
      method: 'POST',
      target: '/api/console/widgets',
      apiBaseUrl: 'http://127.0.0.1:7800',
      account: null,
      password: null,
      body: '{"name":"demo"}',
      bodyFile: null,
      headers: [],
      outDir: outputDir,
      expectStatus: 201,
      printBody: false,
      help: false,
    },
    {
      repoRoot: '/repo',
      loadRootCredentials: () => ({
        account: 'root',
        password: 'change-me',
        envFilePath: '/repo/api/apps/api-server/.env',
      }),
      fetchImpl: async (url, init) => {
        calls.push({ url, init });
        if (String(url).endsWith('/api/public/auth/sign-in')) {
          return createJsonResponse({
            status: 200,
            headers: { 'set-cookie': 'oneflowbase_session=session-secret; Path=/; HttpOnly' },
            body: { data: { csrf_token: 'csrf-token', current_workspace_id: 'workspace-1' } },
          });
        }

        return createJsonResponse({
          status: 201,
          headers: { 'content-type': 'application/json' },
          body: { data: { id: 'widget-1' } },
        });
      },
      now: () => new Date('2026-06-18T12:00:00.000Z'),
    }
  );

  assert.equal(result.ok, true);
  assert.equal(result.status, 201);
  assert.equal(String(calls[0].url), 'http://127.0.0.1:7800/api/public/auth/sign-in');
  assert.equal(calls[0].init.body, JSON.stringify({
    authenticator_id: '00000000-0000-0000-0000-000000000001',
    identifier: 'root',
    password: 'change-me',
  }));
  assert.equal(calls[1].init.headers.cookie, 'oneflowbase_session=session-secret');
  assert.equal(calls[1].init.headers['x-csrf-token'], 'csrf-token');
  assert.equal(calls[1].init.headers['content-type'], 'application/json');
  assert.equal(calls[1].init.body, '{"name":"demo"}');

  const evidence = JSON.parse(fs.readFileSync(result.evidencePath, 'utf8'));
  assert.equal(evidence.auth.account, 'root');
  assert.equal(evidence.auth.envFilePath, '/repo/api/apps/api-server/.env');
  assert.equal(evidence.request.headers.cookie, '[redacted]');
  assert.equal(evidence.request.headers['x-csrf-token'], '[redacted]');
  assert.equal(evidence.response.status, 201);
  assert.deepEqual(JSON.parse(fs.readFileSync(result.responseBodyPath, 'utf8')), {
    data: { id: 'widget-1' },
  });
});

test('runApiDebug marks expected status mismatch without throwing away evidence', async () => {
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-api-debug-'));

  const result = await runApiDebug(
    {
      method: 'GET',
      target: '/api/console/me',
      apiBaseUrl: 'http://127.0.0.1:7800',
      account: 'root',
      password: 'change-me',
      body: null,
      bodyFile: null,
      headers: [],
      outDir: outputDir,
      expectStatus: 200,
      printBody: false,
      help: false,
    },
    {
      repoRoot: '/repo',
      loadRootCredentials: () => ({
        account: 'root',
        password: 'change-me',
        envFilePath: '/repo/api/apps/api-server/.env',
      }),
      fetchImpl: async (url) => {
        if (String(url).endsWith('/api/public/auth/sign-in')) {
          return createJsonResponse({
            status: 200,
            headers: { 'set-cookie': 'oneflowbase_session=session-secret; Path=/' },
            body: { data: { csrf_token: 'csrf-token' } },
          });
        }

        return createJsonResponse({
          status: 404,
          headers: { 'content-type': 'application/json' },
          body: { error: { code: 'not_found' } },
        });
      },
    }
  );

  assert.equal(result.ok, false);
  assert.equal(result.status, 404);
  assert.match(result.error, /expected status 200/u);
  assert.equal(fs.existsSync(result.evidencePath), true);
});

function createJsonResponse({ status, headers, body }) {
  const normalizedHeaders = new Map(
    Object.entries(headers).map(([name, value]) => [name.toLowerCase(), value])
  );
  const text = JSON.stringify(body);

  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: statusTextFor(status),
    headers: {
      get(name) {
        return normalizedHeaders.get(String(name).toLowerCase()) ?? null;
      },
      forEach(callback) {
        for (const [name, value] of normalizedHeaders.entries()) {
          callback(value, name);
        }
      },
    },
    async text() {
      return text;
    },
  };
}

function statusTextFor(status) {
  if (status === 200) {
    return 'OK';
  }
  if (status === 201) {
    return 'Created';
  }
  if (status === 404) {
    return 'Not Found';
  }
  return '';
}
