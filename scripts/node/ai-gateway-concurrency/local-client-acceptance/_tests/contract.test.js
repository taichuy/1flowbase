'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const {
  CLIENT_PROTOCOLS, TEXT_VECTOR, TOOL_VECTOR, buildClientPlan, selectExecutionSurface,
} = require('../contract');

const paths = {
  config: '/tmp/local-client/config',
  output: '/tmp/local-client/output',
  toolFile: '/tmp/local-client/output/tool-vector.txt',
};
const target = { model: 'fixture-model', apiKey: 'sk-test-secret-value', gatewayBaseUrl: 'http://127.0.0.1:4567' };

test('AC-009 fixes Claude Anthropic SSE and OpenCode Chat SSE commands/config', () => {
  const claude = buildClientPlan('claude', '/machine/claude', target, paths, TOOL_VECTOR, 'anthropic_sse');
  assert.equal(claude.invocation.executable, '/machine/claude');
  assert.ok(claude.invocation.args.includes('Read'));
  assert.equal(claude.environment.ANTHROPIC_BASE_URL, target.gatewayBaseUrl);
  assert.equal(claude.configFiles[0].path, path.join(paths.config, 'settings.json'));

  const opencode = buildClientPlan(
    'opencode', '/machine/opencode', target, paths, TEXT_VECTOR, 'openai_chat_sse',
  );
  const config = JSON.parse(opencode.environment.OPENCODE_CONFIG_CONTENT);
  assert.equal(config.provider.oneflowbase_local_acceptance.npm, '@ai-sdk/openai-compatible');
  assert.equal(config.provider.oneflowbase_local_acceptance.options.baseURL, `${target.gatewayBaseUrl}/v1`);
  assert.deepEqual(CLIENT_PROTOCOLS.opencode, ['openai_chat_sse']);
});

test('AC-009 Codex Responses SSE and WebSocket plans differ only in explicit websocket support', () => {
  const sse = buildClientPlan('codex', '/machine/codex', target, paths, TEXT_VECTOR, 'responses_sse');
  const websocket = buildClientPlan(
    'codex', '/machine/codex', target, paths, TEXT_VECTOR, 'responses_websocket',
  );
  assert.ok(sse.invocation.args.includes('model_providers.oneflowbase_local_acceptance.supports_websockets=false'));
  assert.ok(websocket.invocation.args.includes('model_providers.oneflowbase_local_acceptance.supports_websockets=true'));
  assert.ok(sse.invocation.args.includes('model_providers.oneflowbase_local_acceptance.wire_api="responses"'));
  assert.deepEqual(CLIENT_PROTOCOLS.codex, ['responses_sse', 'responses_websocket']);
});

test('AC-009 selects only an available tmux or ACP-headless surface', () => {
  assert.deepEqual(selectExecutionSurface('auto', { tmux: true, acpHeadless: false }), {
    status: 'selected', surface: 'tmux', reason: 'available',
  });
  assert.deepEqual(selectExecutionSurface('acp-headless', { tmux: true, acpHeadless: false }), {
    status: 'skipped', surface: null, reason: 'acp_headless_unavailable',
  });
  assert.equal(
    selectExecutionSurface('auto', { tmux: true, acpHeadless: true }).surface,
    'acp-headless',
  );
});
