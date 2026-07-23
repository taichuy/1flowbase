'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  claudeEnvironment,
  codexEnvironment,
  opencodeEnvironment,
  sanitizedEnvironment,
} = require('../environment');
const { readReadyManifest } = require('../inputs');

function writeManifest(root, gatewayBaseUrl = 'http://127.0.0.1:41002') {
  const file = path.join(root, 'ready.json');
  fs.writeFileSync(file, JSON.stringify({
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    gateway_base_url: gatewayBaseUrl,
    targets: {
      openai: {
        application_id: 'openai-app', model: 'fixture-model', api_key: 'sk-openai',
        gateway: { base_url: gatewayBaseUrl },
      },
      anthropic: {
        application_id: 'anthropic-app', model: 'fixture-model', api_key: 'sk-anthropic',
        gateway: { base_url: gatewayBaseUrl },
      },
    },
  }));
  return file;
}

// Root #1377 AC-006/008: only a WP3 loopback manifest may select sentinel targets.
test('controlled negatives reject external or mismatched gateway origins', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-smoke-inputs-'));
  try {
    assert.throws(() => readReadyManifest(writeManifest(root, 'https://api.openai.com')), /loopback/u);
    const file = writeManifest(root);
    const value = JSON.parse(fs.readFileSync(file));
    value.targets.anthropic.gateway.base_url = 'http://127.0.0.1:49999';
    fs.writeFileSync(file, JSON.stringify(value));
    assert.throws(() => readReadyManifest(file), /origin mismatch/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

// Root #1377 AC-006: malicious parent credentials/config canaries must not reach either child.
test('narrow child environments exclude malicious parent canaries and inject only ephemeral keys', () => {
  const parent = {
    PATH: '/safe/bin',
    HOME: '/home/real-user',
    OPENAI_API_KEY: 'real-openai-canary',
    ANTHROPIC_AUTH_TOKEN: 'real-anthropic-canary',
    CLAUDE_CODE_OAUTH_TOKEN: 'real-oauth-canary',
    AWS_SECRET_ACCESS_KEY: 'real-aws-canary',
    EVIL_PARENT_CANARY: 'must-not-pass',
  };
  const paths = { home: '/tmp/isolated-home', config: '/tmp/isolated-config' };
  const codex = codexEnvironment(parent, paths, 'ephemeral-openai-key');
  const claude = claudeEnvironment(parent, paths, 'http://127.0.0.1:41002', 'ephemeral-anthropic-key');
  const opencode = opencodeEnvironment(parent, paths, 'http://127.0.0.1:41002', {
    model: 'fixture-model', api_key: 'ephemeral-openai-key',
  });
  for (const env of [codex, claude, opencode]) {
    assert.equal(env.HOME, paths.home);
    assert.equal(env.EVIL_PARENT_CANARY, undefined);
    assert.equal(env.ANTHROPIC_AUTH_TOKEN, '');
    assert.equal(env.CLAUDE_CODE_OAUTH_TOKEN, '');
    assert.equal(env.AWS_SECRET_ACCESS_KEY, '');
  }
  assert.equal(codex.OPENAI_API_KEY, '');
  assert.equal(codex.ONEFLOWBASE_APPLICATION_API_KEY, 'ephemeral-openai-key');
  assert.equal(claude.ANTHROPIC_API_KEY, 'ephemeral-anthropic-key');
  assert.equal(claude.ANTHROPIC_BASE_URL, 'http://127.0.0.1:41002');
  const opencodeConfig = JSON.parse(opencode.OPENCODE_CONFIG_CONTENT);
  assert.equal(opencodeConfig.provider.oneflowbase_gateway.npm, '@ai-sdk/openai-compatible');
  assert.equal(opencodeConfig.provider.oneflowbase_gateway.options.baseURL, 'http://127.0.0.1:41002/v1');
  assert.equal(sanitizedEnvironment(opencode).OPENCODE_CONFIG_CONTENT, '<isolated-config>');
});
