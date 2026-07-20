'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const {
  SERVICE_LOG_BYTE_CAP,
  persistServiceLogs,
  redactServiceLog,
} = require('../service-logs');

test('AC service logs: independent stdout/stderr sections are tail-capped and secret canaries are redacted', () => {
  const artifactRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'service-logs-'));
  const canaries = [
    'postgres://db-user:db-password@127.0.0.1/fixture',
    'db-password',
    'bootstrap-password-canary',
    'master-key-canary',
    'gateway_session=cookie-canary',
    'fixture-openai-token',
    'fixture-anthropic-token',
    'sk-application-key-canary',
  ];
  const raw = `${'x'.repeat(SERVICE_LOG_BYTE_CAP)} ${canaries.join(' ')}`;
  try {
    const paths = persistServiceLogs({
      artifactRoot,
      services: {
        'api-server': { stdout: () => raw, stderr: () => raw },
        'plugin-runner': { stdout: () => raw, stderr: () => raw },
      },
      secrets: canaries,
    });
    for (const filePath of Object.values(paths)) {
      const bytes = fs.readFileSync(filePath);
      const value = bytes.toString('utf8');
      assert.equal(bytes.length <= SERVICE_LOG_BYTE_CAP, true);
      assert.match(value, /## stdout/u);
      assert.match(value, /## stderr/u);
      assert.match(value, /\[REDACTED\]/u);
      for (const canary of canaries) assert.equal(value.includes(canary), false);
    }
    assert.equal(redactServiceLog('Authorization: Bearer unlisted-canary').includes('unlisted-canary'), false);
  } finally {
    fs.rmSync(artifactRoot, { recursive: true, force: true });
  }
});

test('AC service logs controlled failure: a failed service write does not skip the other service', () => {
  const writes = [];
  assert.throws(() => persistServiceLogs({
    artifactRoot: '/bounded-fixture-artifacts',
    services: {
      'api-server': { stdout: () => 'api', stderr: () => '' },
      'plugin-runner': { stdout: () => 'runner', stderr: () => '' },
    },
    fsImpl: {
      mkdirSync() {},
      writeFileSync(filePath) {
        writes.push(path.basename(filePath));
        if (filePath.includes('api-server')) throw new Error('controlled write failure');
      },
    },
  }), /service log persistence failed/u);
  assert.deepEqual(writes, ['service-api-server.log', 'service-plugin-runner.log']);
});
