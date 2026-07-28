'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const {
  CLAUDE_PROTOCOL_VECTOR, CLIENT_PROTOCOLS, CONTINUITY_VECTOR, LONG_TEXT_VECTOR,
  PARALLEL_TOOL_VECTOR, PROVIDER_ERROR_VECTOR, SEQUENTIAL_TOOL_VECTOR,
  TEXT_SENTINEL, TEXT_VECTOR, TOOL_RESULT_SENTINEL, TOOL_VECTOR, VECTOR_MANIFEST,
  buildClientPlan, promptFor, selectExecutionSurface, targetsFromReady, vectorsFor,
} = require('../contract');
const {
  LONG_REPEATED_UNICODE_TEXT, PARALLEL_RESULT_A, PARALLEL_RESULT_B,
  PROVIDER_ERROR_BODY, SEQUENTIAL_RESULT_A, SEQUENTIAL_RESULT_B, VECTOR_MANIFEST_SCHEMA,
} = require('../vector-manifest');

const paths = {
  config: '/tmp/local-client/config',
  output: '/tmp/local-client/output',
  toolFile: '/tmp/local-client/output/tool-vector.txt',
  toolAssets: {
    TOOL_PATH: '/tmp/local-client/output/tool-vector.txt',
    PARALLEL_A_PATH: '/tmp/local-client/output/parallel-a.txt',
    PARALLEL_B_PATH: '/tmp/local-client/output/parallel-b.txt',
    SEQUENTIAL_A_PATH: '/tmp/local-client/output/sequential-a.txt',
    SEQUENTIAL_B_PATH: '/tmp/local-client/output/sequential-b.txt',
  },
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

test('WP-14A uses the canonical mock text and two-turn tool sentinels', () => {
  assert.equal(promptFor({ kind: 'text', prompt: `Reply with exactly: ${TEXT_SENTINEL}` }),
    `Reply with exactly: ${TEXT_SENTINEL}`);
  const prompt = promptFor(TOOL_VECTOR, paths.toolFile);
  assert.match(prompt, /1flowbase-client-tool-vector/u);
  assert.match(prompt, /TOOL_VECTOR_PATH=\/tmp\/local-client\/output\/tool-vector\.txt/u);
  assert.equal(TOOL_RESULT_SENTINEL, '1flowbase-client-tool-result');
});

test('AC-009 Codex Responses SSE and WebSocket plans differ only in explicit websocket support', () => {
  const sse = buildClientPlan('codex', '/machine/codex', target, paths, TEXT_VECTOR, 'responses_sse');
  const websocket = buildClientPlan(
    'codex', '/machine/codex', target, paths, TEXT_VECTOR, 'responses_websocket',
  );
  assert.ok(sse.invocation.args.includes('model_providers.oneflowbase_local_acceptance.supports_websockets=false'));
  assert.ok(websocket.invocation.args.includes('model_providers.oneflowbase_local_acceptance.supports_websockets=true'));
  assert.equal(websocket.environment.RUST_LOG, 'codex_core::client=info');
  assert.ok(sse.invocation.args.includes('model_providers.oneflowbase_local_acceptance.wire_api="responses"'));
  assert.deepEqual(CLIENT_PROTOCOLS.codex, ['responses_sse', 'responses_websocket']);
});

test('WP-D4C maps protocol-matched published fixture applications to three clients', () => {
  const provider = (code) => ({
    application_id: `${code}-app`, model: 'fixture-model', api_key: `${code}-secret`,
    gateway: { base_url: 'http://127.0.0.1:7800' },
    durable: { list_runs: {}, query_run: {} }, runtime_activity: {}, plugin_runner_active_streams: {},
  });
  const targets = targetsFromReady({
    schema_version: '1flowbase.ai-gateway-fixture/v1',
    gateway_base_url: 'http://127.0.0.1:7800',
    targets: {
      openai: provider('openai'),
      anthropic: provider('anthropic'),
      openai_compatible: provider('openai_compatible'),
    },
  });
  assert.equal(targets.codex.applicationId, 'openai-app');
  assert.equal(targets.opencode.apiKey, 'openai_compatible-secret');
  assert.equal(targets.claude.applicationId, 'anthropic-app');
  assert.equal(targets.codex.gatewayBaseUrl, 'http://127.0.0.1:7800');
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

test('WP-D4B fixes a finite deterministic vector manifest for all three machine clients', () => {
  assert.equal(VECTOR_MANIFEST.schema_version, VECTOR_MANIFEST_SCHEMA);
  assert.deepEqual(VECTOR_MANIFEST.vectors.map((vector) => vector.id), [
    TEXT_VECTOR.id,
    TOOL_VECTOR.id,
    LONG_TEXT_VECTOR.id,
    CONTINUITY_VECTOR.id,
    PROVIDER_ERROR_VECTOR.id,
    PARALLEL_TOOL_VECTOR.id,
    SEQUENTIAL_TOOL_VECTOR.id,
    CLAUDE_PROTOCOL_VECTOR.id,
  ]);
  for (const vector of VECTOR_MANIFEST.vectors) {
    assert.equal(vector.expected.gateway_executor_invocations, 0);
    assert.equal(vector.expected.network_observer_outbound, 0);
    assert.equal(
      Number.isInteger(vector.expected.provider_requests)
        || Number.isInteger(vector.expected.minimum_provider_requests),
      true,
    );
    assert.equal(vector.expected.success_terminal_counts.length, 1);
  }
  assert.equal(PROVIDER_ERROR_VECTOR.expected.error_body, PROVIDER_ERROR_BODY);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.provider_requests, undefined);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.minimum_provider_requests, 1);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.durable_runs, 'provider_requests');
  assert.equal(vectorsFor('codex', 'responses_websocket').includes(PROVIDER_ERROR_VECTOR), false);
  assert.equal(vectorsFor('claude', 'anthropic_sse').includes(CLAUDE_PROTOCOL_VECTOR), true);
  assert.equal(vectorsFor('opencode', 'openai_chat_sse').includes(CLAUDE_PROTOCOL_VECTOR), false);
});

test('WP-D4B pins exact long Unicode and callback grouping expectations', () => {
  assert.equal(LONG_TEXT_VECTOR.expected.assistant_texts[0], LONG_REPEATED_UNICODE_TEXT);
  assert.ok(Buffer.byteLength(LONG_REPEATED_UNICODE_TEXT) > 32 * 1024);
  assert.match(LONG_REPEATED_UNICODE_TEXT, /重复段🙂🚀/u);
  assert.match(LONG_REPEATED_UNICODE_TEXT, /e\u0301/u);
  assert.deepEqual(PARALLEL_TOOL_VECTOR.expected.tool_result_markers, [PARALLEL_RESULT_A, PARALLEL_RESULT_B]);
  assert.equal(PARALLEL_TOOL_VECTOR.expected.minimum_provider_requests, 2);
  assert.equal(PARALLEL_TOOL_VECTOR.expected.callback_resumes, 1);
  assert.equal(PARALLEL_TOOL_VECTOR.expected.minimum_callback_resumes, undefined);
  assert.deepEqual(SEQUENTIAL_TOOL_VECTOR.expected.tool_result_markers, [
    SEQUENTIAL_RESULT_A, SEQUENTIAL_RESULT_B,
  ]);
  assert.equal(SEQUENTIAL_TOOL_VECTOR.expected.minimum_provider_requests, 3);
  assert.equal(SEQUENTIAL_TOOL_VECTOR.expected.tool_call_count, 2);
  assert.equal(SEQUENTIAL_TOOL_VECTOR.expected.minimum_callback_resumes, 2);
});

test('WP-D4B records only observed Claude profile evidence without injecting it into Codex or OpenCode', () => {
  const claude = buildClientPlan(
    'claude', '/machine/claude', target, paths, CLAUDE_PROTOCOL_VECTOR, 'anthropic_sse',
  );
  assert.ok(claude.invocation.args.includes('claude-opus-4-6[1m]'));
  assert.ok(claude.invocation.args.includes('--effort'));
  assert.ok(claude.invocation.args.includes('high'));
  assert.equal(claude.environment.USE_API_CONTEXT_MANAGEMENT, '1');
  assert.deepEqual(CLAUDE_PROTOCOL_VECTOR.protocol_profile.expected_evidence, {
    configured_model: 'claude-opus-4-6[1m]',
    base_model: 'claude-opus-4-6',
    thinking_type: 'adaptive',
    context_management: true,
  });
  assert.deepEqual(CLAUDE_PROTOCOL_VECTOR.expected.request_body_keys, [
    'context_management', 'thinking',
  ]);
  assert.equal(CLAUDE_PROTOCOL_VECTOR.expected.request_body_model, 'claude-opus-4-6');
  assert.doesNotMatch(promptFor(CLAUDE_PROTOCOL_VECTOR, paths), /output_config|effort/u);

  const codex = buildClientPlan('codex', '/machine/codex', target, paths, TEXT_VECTOR, 'responses_sse');
  const opencode = buildClientPlan(
    'opencode', '/machine/opencode', target, paths, TEXT_VECTOR, 'openai_chat_sse',
  );
  for (const plan of [codex, opencode]) {
    assert.doesNotMatch(JSON.stringify(plan), /context_management|adaptive|\[1m\]/u);
  }
});

test('BLO-05 leaves Claude retry ownership visible to the raw-error vector', () => {
  const errorPlan = buildClientPlan(
    'claude', '/machine/claude', target, paths, PROVIDER_ERROR_VECTOR, 'anthropic_sse',
  );
  const textPlan = buildClientPlan(
    'claude', '/machine/claude', target, paths, TEXT_VECTOR, 'anthropic_sse',
  );
  assert.equal(errorPlan.environment.CLAUDE_CODE_MAX_RETRIES, undefined);
  assert.equal(textPlan.environment.CLAUDE_CODE_MAX_RETRIES, undefined);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.error_body, PROVIDER_ERROR_BODY);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.provider_requests, undefined);
  assert.equal(PROVIDER_ERROR_VECTOR.expected.minimum_provider_requests, 1);
  assert.deepEqual(PROVIDER_ERROR_VECTOR.expected.success_terminal_counts, [0]);
});

test('WP-D4B continuity uses each installed client session surface', () => {
  const claudeFirst = buildClientPlan(
    'claude', '/machine/claude', target, paths, CONTINUITY_VECTOR, 'anthropic_sse',
    { turnIndex: 0, sessionId: '11111111-1111-4111-8111-111111111111' },
  );
  const claudeSecond = buildClientPlan(
    'claude', '/machine/claude', target, paths, CONTINUITY_VECTOR, 'anthropic_sse',
    { turnIndex: 1, sessionId: '11111111-1111-4111-8111-111111111111' },
  );
  assert.ok(claudeFirst.invocation.args.includes('--session-id'));
  assert.ok(claudeSecond.invocation.args.includes('--resume'));
  assert.equal(claudeFirst.invocation.args.includes('--no-session-persistence'), false);

  const codexFirst = buildClientPlan(
    'codex', '/machine/codex', target, paths, CONTINUITY_VECTOR, 'responses_sse',
  );
  const codexSecond = buildClientPlan(
    'codex', '/machine/codex', target, paths, CONTINUITY_VECTOR, 'responses_sse',
    { turnIndex: 1, sessionId: 'codex-thread' },
  );
  assert.equal(codexFirst.invocation.args.includes('--ephemeral'), false);
  assert.ok(codexSecond.invocation.args.includes('resume'));
  assert.ok(codexSecond.invocation.args.includes('codex-thread'));

  const opencodeSecond = buildClientPlan(
    'opencode', '/machine/opencode', target, paths, CONTINUITY_VECTOR, 'openai_chat_sse',
    { turnIndex: 1, sessionId: 'opencode-session' },
  );
  assert.equal(opencodeSecond.invocation.executable, '/machine/opencode');
  assert.deepEqual(
    opencodeSecond.invocation.args.slice(0, 7),
    [
      'run', '--format', 'json', '--model',
      'oneflowbase_local_acceptance/fixture-model', '--session', 'opencode-session',
    ],
  );
});
