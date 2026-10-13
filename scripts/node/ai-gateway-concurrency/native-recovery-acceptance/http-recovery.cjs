'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const assert = require('node:assert/strict');
const { createHttpRecoveryMock } = require('./http-recovery-mock.cjs');
const { createOwnedGateway } = require('./gateway-owner.cjs');
const { decodeSseChunks } = require('../protocol-oracle/decoder');

function loadWebSocket() {
  const root = process.env.NRA_WEB_REPO || path.resolve(__dirname, '../../../..');
  const pnpm = path.join(root, 'web/node_modules/.pnpm');
  const installed = fs.readdirSync(pnpm).find(name => /^ws@8\./u.test(name));
  assert.ok(installed, 'reuse installed ws@8 dependency');
  return require(path.join(pnpm, installed, 'node_modules/ws')).WebSocket;
}
async function websocketTurn(target, body, session) {
  const WebSocket = loadWebSocket();
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(target.gateway.websocket_url, { headers: {
      authorization: target.gateway.authorization, 'session-id': session,
      'openai-beta': 'responses_websockets=2026-02-06',
    } });
    const events = [];
    const timer = setTimeout(() => { socket.terminate(); reject(new Error('WebSocket fixture timed out')); }, 60_000);
    function finish() { clearTimeout(timer); socket.close(); resolve(events); }
    socket.on('open', () => socket.send(JSON.stringify({ ...body, type: 'response.create' })));
    socket.on('message', data => {
      const event = JSON.parse(data.toString());
      events.push(event);
      if (['response.completed', 'response.failed', 'error'].includes(event.type)) finish();
    });
    socket.on('error', error => { clearTimeout(timer); reject(error); });
    socket.on('close', finish);
  });
}
async function run() {
  const artifactRoot = path.resolve(process.env.NRA_ARTIFACT_DIR || 'tmp/test-governance/responses-1008/http-recovery');
  fs.mkdirSync(artifactRoot, { recursive: true });
  const mock = await createHttpRecoveryMock();
  let gateway;
  const evidence = { model: 'gpt-6-luna', reasoning_effort: 'max', controlled_mock: true };
  try {
    gateway = await createOwnedGateway({ upstreamBaseUrl: mock.baseUrl,
      artifactRoot: path.join(artifactRoot, 'service-logs'), responsesTransportPolicy: 'inherit' });
    const session = crypto.randomUUID();
    const body = { model: 'gpt-6-luna', reasoning: { effort: 'max' }, store: false,
      instructions: 'Protocol fixture. Do not execute any tools.',
      input: [{ role: 'user', content: [{ type: 'input_text', text: 'Keep this full input through HTTP recovery.' }] }],
      tools: [{ type: 'function', name: 'fixture', description: 'Never execute',
        parameters: { type: 'object', properties: { text: { type: 'string' } }, required: ['text'], additionalProperties: false }, strict: true }],
      include: ['reasoning.encrypted_content'], parallel_tool_calls: true };
    evidence.websocket_events = await websocketTurn(gateway.target, body, session);
    const failureRows = execFileSync('docker', ['exec', 'docker-db-1', 'psql', '-U', 'postgres',
      '-d', gateway.databaseName, '-At', '-v', 'ON_ERROR_STOP=1', '-c',
      'select error_payload from flow_runs where error_payload is not null order by started_at'],
    { encoding: 'utf8', maxBuffer: 1024 * 1024 });
    evidence.failures = failureRows.trim().split('\n').filter(Boolean).map(JSON.parse);
    evidence.upstream = mock.trace;
    fs.writeFileSync(path.join(artifactRoot, 'receipt.json'), JSON.stringify(evidence, null, 2));
    assert.ok(evidence.websocket_events.some(e => e.type === 'error' || e.type === 'response.failed'));
    assert.ok(!evidence.websocket_events.some(e => e.type === 'response.completed'), 'physical close cannot fabricate completion');
    assert.ok(!evidence.websocket_events.some(e => /delta|output_item.done/u.test(e.type)), 'failed attempts expose no semantic output');
    const wsCount = mock.trace.length;
    // AI Native cannot prove an opaque request replayable. The provider reports
    // the retryable physical interruption; Codex owns replay and HTTP fallback.
    assert.equal(wsCount, 1, 'NativeOpaque must not be replayed by the host');
    assert.equal(evidence.failures.length, 1);
    const recovery = evidence.failures[0].ai_native_recovery;
    assert.equal(evidence.failures[0].error_code, 'provider_transport_unavailable');
    assert.equal(recovery.decision, 'non_reproducible');
    assert.equal(recovery.provider_attempts_consumed, 1);
    assert.equal(recovery.provider_inner_receipt.disposition, 'logical_invocation_retry');
    assert.equal(recovery.provider_inner_receipt.commit_level, 'lifecycle_only');
    assert.equal(recovery.provider_inner_receipt.reason, 'transport_disconnected');
    for (const previous of [undefined, null]) {
      const retry = { ...body, stream: true, ...(previous === null ? { previous_response_id: null } : {}) };
      const response = await fetch(gateway.target.gateway.responses_url, {
        method: 'POST', headers: { authorization: gateway.target.gateway.authorization,
          'content-type': 'application/json', 'session-id': session, 'x-codex-turn-id': `http-${previous === null ? 'null' : 'absent'}` },
        body: JSON.stringify(retry), signal: AbortSignal.timeout(60_000),
      });
      const raw = await response.text();
      assert.equal(response.status, 200, raw.slice(0, 300));
      const events = decodeSseChunks([Buffer.from(raw)])
        .filter(record => !record.done).map(record => record.data);
      const completed = events.filter(e => e.type === 'response.completed');
      assert.equal(completed.length, 1, 'HTTP retry must complete exactly once');
      assert.ok(!events.some(e => e.type === 'response.failed' || e.type === 'error'));
      const upstream = mock.trace.at(-1);
      assert.equal(upstream.transport, 'http', 'HTTP fallback cannot repeat upstream WebSocket failure');
      for (const key of ['model', 'reasoning', 'instructions', 'input', 'tools', 'include', 'parallel_tool_calls', 'store']) {
        assert.deepEqual(upstream.body[key], retry[key], `raw protocol fidelity: ${key}`);
      }
      evidence[`http_${previous === null ? 'null' : 'absent'}_events`] = events;
    }
    assert.equal(mock.trace.filter(r => r.transport === 'websocket').length, wsCount);
    assert.equal(mock.trace.filter(r => r.transport === 'http').length, 2);
    evidence.upstream = mock.trace;
    assert.ok(!JSON.stringify(evidence).includes('private-canary'), 'peer close reason remains redacted');
    evidence.api_binary_sha256 = crypto.createHash('sha256').update(fs.readFileSync(process.env.NRA_API_BINARY)).digest('hex');
    evidence.plugin_package_sha256 = crypto.createHash('sha256').update(fs.readFileSync(process.env.NRA_PLUGIN_PACKAGE)).digest('hex');
    fs.writeFileSync(path.join(artifactRoot, 'receipt.json'), JSON.stringify(evidence, null, 2));
    console.log(JSON.stringify({ pass: true, ws_attempts: wsCount, http_completions: 2, artifactRoot }));
  } finally {
    try { await gateway?.close(); } finally { await mock.close(); }
  }
}
if (require.main === module) run().catch(error => { console.error(error.stack); process.exitCode = 1; });
module.exports = { run, websocketTurn };
