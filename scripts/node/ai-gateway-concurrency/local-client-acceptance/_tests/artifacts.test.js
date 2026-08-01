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

test('Root #1556 F11 redacts by structure without corrupting schema text with a short DB password', () => {
  const safe = redact({
    schema_version: '1flowbase.local-count-tokens-upgrade-run/v5',
    provider_path: '/opt/1flowbase/providers/deepseek',
    owner_password: 'owner-password',
    primary_error: {
      message: 'database postgres://owner:1flowbase@127.0.0.1/dev rejected owner-password',
    },
  }, {
    credentials: ['owner-password'],
    credentialUrls: ['postgres://owner:1flowbase@127.0.0.1/dev'],
  });

  assert.equal(safe.schema_version, '1flowbase.local-count-tokens-upgrade-run/v5');
  assert.equal(safe.provider_path, '/opt/1flowbase/providers/deepseek');
  assert.equal(safe.owner_password, '<redacted>');
  assert.match(safe.primary_error.message, /postgres:\/\/<redacted>@127\.0\.0\.1\/dev/u);
  assert.doesNotMatch(JSON.stringify(safe), /owner-password|owner:1flowbase/u);
});

test('Root #1556 F14 redacts finite raw, escaped, percent, and collision-ordered variants', () => {
  const shortSecret = 'credential-value';
  const longSecret = 'credential-value-with-suffix';
  const escapedSecret = 'secret-"quoted"\\path/%';
  const jsonEscaped = JSON.stringify(escapedSecret).slice(1, -1);
  const doubleEscaped = JSON.stringify(jsonEscaped).slice(1, -1);
  const percentEncoded = encodeURIComponent(escapedSecret);
  const safe = redact({
    primary_error: {
      message: [
        escapedSecret,
        JSON.stringify({ password: escapedSecret }),
        JSON.stringify(JSON.stringify({ password: escapedSecret })),
        percentEncoded,
        longSecret,
        shortSecret,
      ].join('\n'),
    },
  }, {
    descriptors: [
      { kind: 'credential', value: escapedSecret },
      { kind: 'credential', value: shortSecret },
      { kind: 'credential', value: longSecret },
    ],
  });

  for (const variant of [escapedSecret, jsonEscaped, doubleEscaped, percentEncoded,
    shortSecret, longSecret]) {
    assert.equal(safe.primary_error.message.includes(variant), false, `diagnostic leaked ${variant}`);
  }
  assert.deepEqual(safe.primary_error.message.split('\n').slice(-2), [
    '<redacted>',
    '<redacted>',
  ]);
});

test('Root #1556 F14 classifies dotenv secrets while preserving public material and common text', () => {
  const trustedKeys = JSON.stringify([{
    key_id: 'dotenv-key',
    public_key_pem: 'quoted-json-value',
  }]);
  const schema = '1flowbase.local-count-tokens-upgrade-run/v5';
  const providerPath = '/opt/1flowbase/providers/deepseek';
  const apiKeyId = 'public-api-key-id';
  const secretResolver = 'env';
  const privateValue = 'private-credential-value';
  const diagnostic = JSON.stringify({
    trustedKeys, schema, providerPath, apiKeyId, secretResolver, privateValue,
  });
  const safe = redact({ primary_error: { message: diagnostic } }, {
    descriptors: [
      { kind: 'env', key: 'API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON', value: trustedKeys },
      { kind: 'env', key: 'ACCEPTANCE_SCHEMA', value: schema },
      { kind: 'env', key: 'PROVIDER_PATH', value: providerPath },
      { kind: 'env', key: 'BOOTSTRAP_WORKSPACE_NAME', value: '1flowbase' },
      { kind: 'env', key: 'APPLICATION_API_KEY_ID', value: apiKeyId },
      { kind: 'env', key: 'API_SECRET_RESOLVER', value: secretResolver },
      { kind: 'env', key: 'API_PRIVATE_CREDENTIAL', value: privateValue },
    ],
  });
  const parsed = JSON.parse(safe.primary_error.message);

  assert.equal(parsed.trustedKeys, trustedKeys);
  assert.equal(parsed.schema, schema);
  assert.equal(parsed.providerPath, providerPath);
  assert.equal(parsed.apiKeyId, apiKeyId);
  assert.equal(parsed.secretResolver, secretResolver);
  assert.equal(parsed.privateValue, '<redacted>');
  assert.match(safe.primary_error.message, /dotenv-key|quoted-json-value|1flowbase/u);
});

test('Root #1556 F14 sanitizes DB URL userinfo without replacing common password text', () => {
  const databaseUrl = 'postgres://owner:1flowbase@127.0.0.1:35432/1flowbase';
  const safe = redact({
    schema_version: '1flowbase.local-count-tokens-upgrade-run/v5',
    primary_error: {
      message: `raw=${databaseUrl} encoded=${encodeURIComponent(databaseUrl)}`,
    },
  }, {
    descriptors: [{ kind: 'env', key: 'API_DATABASE_URL', value: databaseUrl }],
  });

  assert.equal(safe.schema_version, '1flowbase.local-count-tokens-upgrade-run/v5');
  assert.match(safe.primary_error.message,
    /raw=postgres:\/\/<redacted>@127\.0\.0\.1:35432\/1flowbase/u);
  assert.doesNotMatch(safe.primary_error.message, /owner:1flowbase|postgres%3A/u);
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
