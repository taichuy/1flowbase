'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  clientFrame,
  consumeServerFrames,
  runGatewayWebSocketAcceptance,
} = require('../gateway-websocket');
const { HTTP_500_ERROR_BODY } = require('../../mock-upstream');

const RUN_ID = '018f7af7-3694-7ba0-90bf-83b5ec689705';

function readyManifest() {
  return {
    targets: {
      openai: {
        application_id: 'application-1',
        provider_instance_id: 'provider-1',
        model: 'published-model',
        api_key: 'application-secret',
        gateway: { responses_url: 'http://127.0.0.1:4100/v1/responses' },
        durable: {
          query_run: { url_template: `http://127.0.0.1:4100/api/agent/v1/runs/{run_id}` },
          list_runs: { url: 'http://127.0.0.1:4100/api/console/runs' },
        },
      },
    },
  };
}

function serverFrame(value) {
  const payload = Buffer.from(value);
  if (payload.length < 126) {
    return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  }
  const header = Buffer.alloc(4);
  header[0] = 0x81;
  header[1] = 126;
  header.writeUInt16BE(payload.length, 2);
  return Buffer.concat([header, payload]);
}

function collectedFrameChunks(events) {
  const collected = [];
  const remaining = consumeServerFrames(
    Buffer.concat(events.map((event) => serverFrame(JSON.stringify(event)))),
    (opcode, payload) => {
      assert.equal(opcode, 0x1);
      collected.push([payload]);
    },
  );
  assert.equal(remaining.length, 0);
  return collected;
}

test('Root #1461 WP-14 WebSocket framing is bounded, masked outbound, and incremental inbound', () => {
  const outbound = clientFrame('hello');
  assert.equal((outbound[1] & 0x80) !== 0, true);
  const observed = [];
  const frame = serverFrame('{"type":"response.completed"}');
  let buffered = consumeServerFrames(frame.subarray(0, 4), (_opcode, payload) => observed.push(payload.toString()));
  buffered = consumeServerFrames(Buffer.concat([buffered, frame.subarray(4)]), (_opcode, payload) => observed.push(payload.toString()));
  assert.equal(buffered.length, 0);
  assert.deepEqual(observed, ['{"type":"response.completed"}']);
});

test('Root #1461 WP-14 connects Gateway WS trace to durable and WireAudit evidence', async () => {
  const snapshots = [
    { counters: {}, entries: [] },
    {
      counters: {},
      entries: [{ sequence: 1, event: 'arrival', transport: 'responses-sse', nonce: 'mock-000001', request: { body: { model: 'published-model' } } }],
    },
  ];
  const result = await runGatewayWebSocketAcceptance({
    ready: readyManifest(),
    mockSnapshot: () => snapshots.shift(),
  }, {
    async collectGatewayFrames(_target, _clientTraceId, options = {}) {
      const isError = options.inputText !== undefined;
      return collectedFrameChunks(isError ? [
        { type: 'response.created', response: { id: `resp_${RUN_ID}` } },
        {
          type: 'response.failed',
          response: {
            id: `resp_${RUN_ID}`,
            error: { message: HTTP_500_ERROR_BODY, code: 'provider_upstream_error' },
          },
        },
      ] : [
        { type: 'response.created', response: { id: `resp_${RUN_ID}` } },
        { type: 'response.output_text.delta', delta: 'mock-000001:chunk-a' },
        { type: 'response.completed', response: { id: `resp_${RUN_ID}` } },
      ]);
    },
    async queryDurableRun(_target, trace) {
      return {
        run: {
          id: trace.run_id,
          status: trace.terminal_type === 'response.failed' ? 'failed' : 'succeeded',
          ...(trace.terminal_type === 'response.failed'
            ? { error_message: HTTP_500_ERROR_BODY }
            : {}),
        },
        digest_sha256: 'a'.repeat(64),
      };
    },
  });
  assert.equal(result.trace.run_id, RUN_ID);
  assert.equal(result.durable.run.status, 'succeeded');
  assert.equal(result.wire_audit.verdict, 'PASS');
  assert.equal(result.error_fidelity.trace.error_message, HTTP_500_ERROR_BODY);
  assert.equal(result.error_fidelity.durable.run.error_message, HTTP_500_ERROR_BODY);
  assert.doesNotMatch(JSON.stringify(result), /application-secret/u);
});
