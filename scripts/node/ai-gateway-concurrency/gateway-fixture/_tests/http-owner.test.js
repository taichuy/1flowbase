'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { OwnerHttpClient } = require('../http-owner');

test('owner HTTP client accepts a pre-owned session without signing in', () => {
  const client = new OwnerHttpClient('http://127.0.0.1:9000');
  client.attachSession('session=fixed', 'csrf-fixed');
  assert.equal(client.cookie, 'session=fixed');
  assert.equal(client.csrf, 'csrf-fixed');
  assert.throws(() => client.attachSession('', 'csrf'), /required/u);
});

// Root #1377 AC-001/008: the fixture uses the real cookie/CSRF/multipart owner contract.
test('owner client carries sign-in cookie and CSRF into writes', async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (url.endsWith('/sign-in')) {
      return new Response(JSON.stringify({ data: { csrf_token: 'csrf-1' } }), {
        status: 200,
        headers: { 'set-cookie': 'fixture_session=session-1; HttpOnly; Path=/' },
      });
    }
    return new Response(JSON.stringify({ data: { ok: true } }), { status: 200 });
  };
  const client = new OwnerHttpClient('http://127.0.0.1:9000', fetchImpl);
  await client.signIn('root', 'password');
  await client.write('/api/console/example', 'POST', { value: 1 });
  assert.equal(calls[1].init.headers.cookie, 'fixture_session=session-1');
  assert.equal(calls[1].init.headers['x-csrf-token'], 'csrf-1');
  assert.deepEqual(JSON.parse(calls[1].init.body), { value: 1 });
});

test('package upload sends a multipart file and requires installation identity', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gateway-http-'));
  const archive = path.join(root, 'openai.1flowbasepkg');
  fs.writeFileSync(archive, 'archive');
  try {
    let captured;
    const client = new OwnerHttpClient('http://127.0.0.1:9000', async (_url, init) => {
      captured = init;
      return new Response(JSON.stringify({ data: { installation: { id: 'installation-1' } } }), {
        status: 201,
      });
    });
    client.cookie = 'session=x';
    client.csrf = 'csrf-x';
    const result = await client.uploadPackage(archive);
    assert.equal(result.installation.id, 'installation-1');
    assert.ok(captured.body instanceof FormData);
    assert.equal(captured.headers['x-csrf-token'], 'csrf-x');
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('controlled negative refuses a write before CSRF is established', async () => {
  const client = new OwnerHttpClient('http://127.0.0.1:9000', async () => {
    throw new Error('fetch must not run');
  });
  await assert.rejects(client.write('/api/console/example'), /CSRF token is unavailable/u);
});

test('owner client reports the safe API error message for fixture diagnostics', async () => {
  const client = new OwnerHttpClient('http://127.0.0.1:9000', async () => new Response(
    JSON.stringify({
      status: 400,
      code: 'provider_package',
      message: 'package host contract is incompatible',
    }),
    { status: 400 },
  ));

  await assert.rejects(
    client.read('/api/console/example'),
    /400 provider_package: package host contract is incompatible/u,
  );
});
