'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { normalizeOptions, requireLoopbackUrl, requirePort, requirePostgresUrl } = require('../inputs');
const { parseArgs } = require('../../../cli/ai-gateway-fixture');

// Root #1377 AC-001/008: lifecycle inputs must be explicit and fail closed.
test('controlled negatives reject non-PostgreSQL and non-loopback inputs', () => {
  assert.throws(() => requirePostgresUrl('sqlite:///tmp/test.db'), /must name a PostgreSQL/u);
  assert.throws(() => requireLoopbackUrl('https://api.openai.com/v1'), /plain HTTP on loopback/u);
  assert.throws(() => requireLoopbackUrl('http://127.0.0.1:9000/?token=x'), /query/u);
  assert.throws(() => requireLoopbackUrl('http://127.0.0.1:9000/v1'), /without a path/u);
  assert.throws(() => requirePort('7800', 'api-server port'), /integer between 1 and 65535/u);
  assert.throws(() => requirePort(0, 'api-server port'), /integer between 1 and 65535/u);
  assert.throws(() => requirePort(65_536, 'api-server port'), /integer between 1 and 65535/u);
});

test('CLI accepts the complete explicit environment contract', () => {
  const parsed = parseArgs([], {
    AI_GATEWAY_FIXTURE_DATABASE_URL: 'postgres://fixture/db',
    AI_GATEWAY_FIXTURE_API_SERVER_BIN: '/tmp/api-server',
    AI_GATEWAY_FIXTURE_PLUGIN_RUNNER_BIN: '/tmp/plugin-runner',
    AI_GATEWAY_FIXTURE_OPENAI_PACKAGE: '/tmp/openai.pkg',
    AI_GATEWAY_FIXTURE_ANTHROPIC_PACKAGE: '/tmp/anthropic.pkg',
    AI_GATEWAY_FIXTURE_UPSTREAM_BASE_URL: 'http://127.0.0.1:9123',
  });
  assert.equal(parsed.databaseUrl, 'postgres://fixture/db');
  assert.equal(parsed.upstreamBaseUrl, 'http://127.0.0.1:9123');
});

test('normalization requires executable binaries and exact package files', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gateway-inputs-'));
  try {
    const file = (name, mode = 0o600) => {
      const target = path.join(root, name);
      fs.writeFileSync(target, name, { mode });
      return target;
    };
    const options = normalizeOptions({
      databaseUrl: 'postgres://fixture@127.0.0.1/fixture_db',
      apiServerBin: file('api-server', 0o700),
      pluginRunnerBin: file('plugin-runner', 0o700),
      openaiPackage: file('openai.1flowbasepkg'),
      anthropicPackage: file('anthropic.1flowbasepkg'),
      upstreamBaseUrl: 'http://127.0.0.1:9123/',
    });
    assert.equal(options.upstreamBaseUrl, 'http://127.0.0.1:9123');
    assert.equal(options.apiPort, null);

    const explicitPort = normalizeOptions({
      ...options,
      upstreamBaseUrl: 'http://127.0.0.1:9123/',
      apiPort: 7800,
    });
    assert.equal(explicitPort.apiPort, 7800);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
