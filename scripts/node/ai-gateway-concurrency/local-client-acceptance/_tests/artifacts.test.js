'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { ARTIFACT_SCHEMA } = require('../contract');
const { createTimeline, redact, validateArtifact, writeArtifact } = require('../artifacts');

function validArtifact() {
  return {
    schema_version: ARTIFACT_SCHEMA,
    status: 'pass',
    clients: [{ name: 'codex', status: 'pass', timeline: [] }],
    cleanup: { status: 'pass', errors: [] },
  };
}

test('AC-009 redacts explicit and shaped secrets from structured artifacts', () => {
  const secret = 'sk-machine-acceptance-secret';
  const safe = redact({
    api_key: secret,
    output: `Authorization: Bearer abcdefghi and ${secret}`,
    api_key_env: 'ONEFLOWBASE_APPLICATION_API_KEY',
  }, [secret]);
  assert.equal(safe.api_key, '<redacted>');
  assert.doesNotMatch(JSON.stringify(safe), /sk-machine|abcdefghi/u);
  assert.equal(safe.api_key_env, 'ONEFLOWBASE_APPLICATION_API_KEY');
});

test('AC-009 validates schema and writes a mode-0600 redacted artifact', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-client-artifact-'));
  try {
    const file = path.join(root, 'artifact.json');
    const artifact = validArtifact();
    artifact.clients[0].timeline = [{ event: 'output', text: 'sk-secret-canary' }];
    writeArtifact(file, artifact, ['sk-secret-canary']);
    assert.equal(fs.statSync(file).mode & 0o777, 0o600);
    assert.doesNotMatch(fs.readFileSync(file, 'utf8'), /sk-secret-canary/u);
    assert.equal(validateArtifact(validArtifact()).schema_version, ARTIFACT_SCHEMA);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('AC-009 timeline uses ordered structured monotonic events', () => {
  const values = [10n, 20n, 35n];
  const timeline = createTimeline(() => values.shift());
  timeline.append('client_started');
  timeline.append('client_exited');
  assert.deepEqual(timeline.snapshot().map(({ sequence, elapsed_ns, event }) => ({ sequence, elapsed_ns, event })), [
    { sequence: 1, elapsed_ns: '10', event: 'client_started' },
    { sequence: 2, elapsed_ns: '25', event: 'client_exited' },
  ]);
});
