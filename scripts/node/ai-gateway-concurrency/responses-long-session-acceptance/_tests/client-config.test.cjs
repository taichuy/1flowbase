'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { clientConfig, clientLogFilter, retainStderrLine } = require('../client-config.cjs');

test('normal client policy retains WebSocket and provider default retry budgets', () => {
  const config = clientConfig({ catalogPath: '/tmp/catalog.json', baseUrl: 'http://127.0.0.1:7600/v1' });
  assert.match(config, /supports_websockets = true/);
  assert.match(config, /model = "gpt-6-luna"/);
  assert.match(config, /model_reasoning_effort = "max"/);
  assert.doesNotMatch(config, /request_max_retries|stream_max_retries/);
});

test('catalog path and URL remain quoted configuration values', () => {
  const catalogPath = '/tmp/catalog "quoted".json';
  const baseUrl = 'http://127.0.0.1:7600/v1';
  const config = clientConfig({ catalogPath, baseUrl });
  assert.ok(config.includes(`model_catalog_json = ${JSON.stringify(catalogPath)}\n`));
  assert.ok(config.includes(`base_url = ${JSON.stringify(baseUrl)}\n`));
});

test('official post-sampling trace target is enabled without broad debug logging', () => {
  assert.ok(clientLogFilter.split(',').includes('codex_core::session::turn=trace'));
  assert.ok(clientLogFilter.split(',').includes('codex_core=info'));
  assert.ok(clientLogFilter.split(',').includes('codex_api::endpoint::responses_websocket=debug'));
  assert.ok(clientLogFilter.split(',').includes('codex_otel.agent_communication=trace'));
});

for (const [name, value] of [
  ['warning', 'WARN runtime: provider_physical_connection_close_exhausted'],
  ['error', 'ERROR codex_core: websocket disconnected before response.completed'],
  ['sampling error', 'INFO codex_core::session::turn: sampling_error=transport_unavailable'],
  ['turn error', 'DEBUG codex_core::session::turn: Turn error: transport_unavailable'],
  ['communication', 'TRACE codex_otel.agent_communication: state="receive" kind="result"'],
  ['post sampling', 'TRACE thread{id="parent"}:turn{turn.id="turn"}: codex_core::session::turn: post sampling token usage model_needs_follow_up=true'],
  ['sample new', 'DEBUG thread{thread.id="parent"}:turn{turn.id="turn"}:responses_websocket.stream_request{}: codex_api::endpoint::responses_websocket: new'],
  ['sample close', 'DEBUG responses_websocket.stream_request{}: codex_api::endpoint::responses_websocket: close time.busy=10ms time.idle=2s'],
  ['connection', 'DEBUG codex_api::endpoint::responses_websocket: successfully connected to websocket: ws://127.0.0.1:7600/v1/responses'],
]) {
  test(`retain complete original ${name} evidence including ANSI`, () => {
    assert.equal(retainStderrLine(`2026-09-30T12:00:00.000Z ${value}`), true);
    assert.equal(retainStderrLine(`\x1b[2m2026-09-30T12:00:00.000Z\x1b[0m ${value}`), true);
  });
}

test('plain setup and crash diagnostics are not silently discarded', () => {
  assert.equal(retainStderrLine('thread panicked: failed to initialize client'), true);
  assert.equal(retainStderrLine('codex: command not found'), true);
});

for (const value of [
  'TRACE codex_core::session::turn: post sampling token estimate estimated_token_count=1',
  'DEBUG codex_api::endpoint::responses_websocket: sending websocket request {"input":[]}',
  'DEBUG responses_websocket.stream_request{}: codex_api::endpoint::responses_websocket: enter',
  'DEBUG responses_websocket.stream_request{}: codex_api::endpoint::responses_websocket: exit',
  'INFO codex_core::session::turn: unrelated lifecycle event',
]) {
  test(`discard irrelevant trace detail: ${value}`, () => {
    assert.equal(retainStderrLine(`2026-09-30T12:00:00.000Z ${value}`), false);
  });
}
