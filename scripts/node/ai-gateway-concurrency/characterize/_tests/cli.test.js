'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const { TRANSPORT } = require('../../contracts');
const { parseCliArgs } = require('../../../cli/ai-gateway-concurrency');

test('AC-007: direct mock CLI selects characterize and fixed repo-root output ownership', () => {
  assert.deepEqual(parseCliArgs([
    '--profile', 'characterize',
    '--mode', 'direct-mock',
    '--repo-root', '/repo',
    '--timeout-ms', '1000',
  ]), {
    help: false,
    mode: 'direct-mock',
    repoRoot: path.resolve('/repo'),
    timeoutMs: 1000,
  });
});

test('AC-003/007: gateway CLI reads credentials only from the named environment variable', () => {
  const parsed = parseCliArgs([
    '--profile', 'characterize',
    '--mode', 'gateway',
    '--responses-sse-url', 'http://127.0.0.1:7800/v1/responses',
    '--mock-responses-websocket-url', 'ws://127.0.0.1:7802/v1/responses',
    '--anthropic-sse-url', 'http://127.0.0.1:7801/v1/messages',
    '--api-key-env', 'FIXTURE_API_KEY',
  ], { FIXTURE_API_KEY: 'fixture-secret' });
  assert.equal(parsed.authorizationToken, 'fixture-secret');
  assert.deepEqual(parsed.endpointSet, {
    [TRANSPORT.RESPONSES_SSE]: 'http://127.0.0.1:7800/v1/responses',
    [TRANSPORT.RESPONSES_WEBSOCKET]: 'ws://127.0.0.1:7802/v1/responses',
    [TRANSPORT.ANTHROPIC_SSE]: 'http://127.0.0.1:7801/v1/messages',
  });
});

test('AC-007 controlled negatives: regression profile and incomplete endpoints fail closed', () => {
  assert.throws(
    () => parseCliArgs(['--profile', 'regression', '--mode', 'direct-mock']),
    /no regression budget is approved/u,
  );
  assert.throws(
    () => parseCliArgs(['--profile', 'characterize', '--mode', 'gateway']),
    /missing required argument/u,
  );
});
