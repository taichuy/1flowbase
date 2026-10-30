// Real gateway/PostgreSQL/plugin regression; only the loopback upstream is synthetic.
'use strict';
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { createOwnedGateway } = require('./gateway-owner.cjs');

async function main() {
  const concurrency = Number(process.env.NRA_CALLBACK_CONCURRENCY || 1);
  assert.ok(Number.isSafeInteger(concurrency) && concurrency > 0, 'positive fixture concurrency');
  const artifactRoot = path.resolve(process.env.NRA_ARTIFACT_DIR || 'tmp/test-governance/native-http-callback');
  fs.mkdirSync(artifactRoot, { recursive: true });
  const trace = [], sockets = new Set(), seeds = new Map(), mockErrors = [];
  let upgrades = 0, sequenceId = 0, gateway;
  // Deterministic overlap, not a timing-based concurrency assertion.
  const phases = Object.fromEntries(['seed', 'resume'].map(name => {
    let release;
    const ready = new Promise(resolve => { release = resolve; });
    return [name, { arrived: 0, active: 0, peak: 0, ready, release }];
  }));
  const server = http.createServer((req, res) => {
    async function handle() {
      if (req.url === '/v1/models') {
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ data: [{ id: 'gpt-6-luna', object: 'model' }] }));
        return;
      }
      if (req.method !== 'POST' || req.url !== '/v1/responses') {
        res.writeHead(404).end(); return;
      }
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const body = JSON.parse(Buffer.concat(chunks).toString());
      const phaseName = body.previous_response_id ? 'resume' : 'seed';
      const phase = phases[phaseName], ordinal = ++sequenceId;
      trace.push({ transport: 'http', phase: phaseName, model: body.model,
        previous_response_id: body.previous_response_id,
        input_types: Array.isArray(body.input) ? body.input.map(x => x.type) : [],
        tool_choice: body.tool_choice, reasoning: body.reasoning });
      phase.arrived++; phase.active++; phase.peak = Math.max(phase.peak, phase.active);
      if (phase.arrived === concurrency) phase.release();
      try {
        assert.ok(phase.arrived <= concurrency, 'no blind upstream replay');
        if (phaseName === 'resume') {
          assert.ok(seeds.has(body.previous_response_id), 'host cursor translated to owned upstream cursor');
          assert.deepEqual(body.input.map(x => x.type), ['function_call_output']);
          assert.equal(body.input[0].call_id, seeds.get(body.previous_response_id));
        }
        await phase.ready;
        const id = `resp_http_callback_${ordinal}`;
        const item = phaseName === 'seed'
          ? { type: 'function_call', id: `fc_http_${ordinal}`, call_id: `call_http_${ordinal}`,
              name: 'callback_probe', arguments: '{}', status: 'completed' }
          : { type: 'message', id: `msg_http_${ordinal}`, role: 'assistant', status: 'completed',
              content: [{ type: 'output_text', text: 'Synthetic callback accepted.' }] };
        if (phaseName === 'seed') seeds.set(id, item.call_id);
        let sequence = 0;
        res.writeHead(200, { 'content-type': 'text/event-stream' });
        const emit = event => res.write(`data: ${JSON.stringify({ ...event, sequence_number: sequence++ })}\n\n`);
        emit({ type: 'response.created', response: { id, status: 'in_progress', output: [] } });
        emit({ type: 'response.output_item.added', output_index: 0, item: { ...item, status: 'in_progress' } });
        if (phaseName === 'seed') {
          emit({ type: 'response.function_call_arguments.delta', output_index: 0, item_id: item.id, delta: '{}' });
          emit({ type: 'response.function_call_arguments.done', output_index: 0, item_id: item.id, arguments: '{}' });
        } else {
          emit({ type: 'response.output_text.delta', output_index: 0, item_id: item.id, content_index: 0, delta: item.content[0].text });
          emit({ type: 'response.output_text.done', output_index: 0, item_id: item.id, content_index: 0, text: item.content[0].text });
        }
        emit({ type: 'response.output_item.done', output_index: 0, item });
        emit({ type: 'response.completed', response: { id, status: 'completed', model: body.model,
          output: [item], usage: { input_tokens: 10, output_tokens: 5, total_tokens: 15 } } });
        res.end('data: [DONE]\n\n');
      } finally { phase.active--; }
    }
    handle().catch(error => {
      mockErrors.push({ name: error.name, message: error.message });
      if (!res.headersSent) res.writeHead(500, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: 'controlled_mock_assertion_failed' }));
    });
  });
  server.on('connection', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
  server.on('upgrade', (_req, socket) => { upgrades++; socket.end('HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n'); });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const evidence = { model: 'gpt-6-luna', reasoning_effort: 'max', controlled_mock: true,
    concurrency, started_at: new Date().toISOString(), requests: [], pass: false };
  try {
    gateway = await createOwnedGateway({ upstreamBaseUrl: `http://127.0.0.1:${server.address().port}`,
      artifactRoot: path.join(artifactRoot, 'service'), responsesTransportPolicy: 'inherit' });
    evidence.gateway_pid = gateway.gatewayPid;
    const post = async (body, session, client, phase) => {
      const started = performance.now();
      const response = await fetch(gateway.target.gateway.responses_url, {
        method: 'POST', headers: { authorization: gateway.target.gateway.authorization,
          'content-type': 'application/json', 'session-id': session },
        body: JSON.stringify(body), signal: AbortSignal.timeout(45000) });
      const raw = await response.text();
      const events = raw.split(/\r?\n\r?\n/).flatMap(block => {
        const data = block.split(/\r?\n/).filter(x => x.startsWith('data:')).map(x => x.slice(5).trim()).join('\n');
        return !data || data === '[DONE]' ? [] : [JSON.parse(data)];
      });
      const terminals = events.filter(x => ['response.completed', 'response.failed', 'response.incomplete', 'response.cancelled'].includes(x.type));
      evidence.requests.push({ client, phase, http_status: response.status, elapsed_ms: performance.now() - started,
        response_id: terminals[0]?.response?.id, terminal_types: terminals.map(x => x.type),
        error: terminals[0]?.response?.error, event_types: events.map(x => x.type) });
      assert.equal(response.status, 200, `HTTP status ${response.status}`);
      assert.equal(terminals.length, 1, 'terminal exactly once');
      assert.equal(terminals[0].type, 'response.completed', JSON.stringify(terminals[0].response?.error));
      assert.ok(!events.some(x => x.type === 'error'), 'no independent SSE error');
      return { events, response: terminals[0].response };
    };
    const results = await Promise.allSettled(Array.from({ length: concurrency }, async (_, client) => {
      const session = crypto.randomUUID();
      const body = { model: 'gpt-6-luna', reasoning: { effort: 'max' }, stream: true, store: false,
        parallel_tool_calls: false, instructions: 'Controlled callback fixture.', tool_choice: 'auto',
        tools: [{ type: 'function', name: 'callback_probe', description: 'Fixture only', strict: true,
          parameters: { type: 'object', properties: {}, required: [], additionalProperties: false } }],
        input: [{ role: 'user', content: [{ type: 'input_text', text: 'Synthetic fixture input.' }] }] };
      const first = await post(body, session, client, 'seed');
      const call = first.events.find(x => x.type === 'response.output_item.done' && x.item?.type === 'function_call')?.item;
      assert.ok(call?.call_id, 'committed formal function call');
      const second = await post({ ...body, previous_response_id: first.response.id,
        input: [{ type: 'function_call_output', call_id: call.call_id, output: '{"ok":true}' }] }, session, client, 'resume');
      assert.ok(second.events.some(x => x.type === 'response.output_text.delta' && x.delta?.length), 'continuation output received');
    }));
    evidence.client_results = results.map(x => ({ status: x.status, error: x.reason?.message }));
    assert.ok(results.every(x => x.status === 'fulfilled'), 'every concurrent callback must complete');
    assert.equal(trace.length, concurrency * 2, 'one seed and one continuation per client');
    assert.equal(upgrades, 0, 'HTTP predecessors must never attempt WebSocket continuation');
    assert.deepEqual(mockErrors, []);
    for (const phase of Object.values(phases)) assert.equal(phase.peak, concurrency, 'upstream overlap was exercised');
    assert.ok(trace.every(x => x.model === 'gpt-6-luna' && x.reasoning?.effort === 'max' && x.tool_choice === 'auto'));
    evidence.pass = true;
    console.log(JSON.stringify({ pass: true, artifact: artifactRoot, concurrency,
      upstream_http_requests: trace.length, upgrades }));
  } finally {
    evidence.finished_at = new Date().toISOString();
    evidence.upstream = trace; evidence.websocket_upgrades = upgrades; evidence.mock_errors = mockErrors;
    evidence.overlap = Object.fromEntries(Object.entries(phases).map(([name, phase]) => [name, { arrived: phase.arrived, peak: phase.peak }]));
    fs.writeFileSync(path.join(artifactRoot, 'receipt.json'), JSON.stringify(evidence, null, 2));
    try { await gateway?.close(); }
    finally { for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); }
  }
}
main().catch(error => { console.error(error.stack); process.exitCode = 1; });
