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

test('AC-003/007: gateway CLI reads distinct Application keys from two named environment variables', () => {
  const parsed = parseCliArgs([
    '--profile', 'characterize',
    '--mode', 'gateway',
    '--responses-sse-url', 'http://127.0.0.1:7800/v1/responses',
    '--mock-responses-websocket-url', 'ws://127.0.0.1:7802/v1/responses',
    '--anthropic-sse-url', 'http://127.0.0.1:7801/v1/messages',
    '--openai-api-key-env', 'OPENAI_FIXTURE_API_KEY',
    '--anthropic-api-key-env', 'ANTHROPIC_FIXTURE_API_KEY',
    '--openai-model', 'gateway-openai-model',
    '--anthropic-model', 'gateway-anthropic-model',
  ], {
    OPENAI_FIXTURE_API_KEY: 'responses-fixture-secret',
    ANTHROPIC_FIXTURE_API_KEY: 'anthropic-fixture-secret',
  });
  assert.deepEqual(parsed.authorizationTokenByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'responses-fixture-secret',
    [TRANSPORT.CHAT_COMPLETIONS_SSE]: 'responses-fixture-secret',
    [TRANSPORT.ANTHROPIC_SSE]: 'anthropic-fixture-secret',
  });
  assert.deepEqual(parsed.modelByTransport, {
    [TRANSPORT.RESPONSES_SSE]: 'gateway-openai-model',
    [TRANSPORT.CHAT_COMPLETIONS_SSE]: 'gateway-openai-model',
    [TRANSPORT.ANTHROPIC_SSE]: 'gateway-anthropic-model',
  });
  assert.deepEqual(parsed.endpointSet, {
    [TRANSPORT.RESPONSES_SSE]: 'http://127.0.0.1:7800/v1/responses',
    [TRANSPORT.RESPONSES_WEBSOCKET]: 'ws://127.0.0.1:7802/v1/responses',
    [TRANSPORT.CHAT_COMPLETIONS_SSE]: 'http://127.0.0.1:7800/v1/chat/completions',
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

test('AC-003 controlled negatives: ambiguous or identical gateway keys fail closed', () => {
  const base = [
    '--profile', 'characterize',
    '--mode', 'gateway',
    '--responses-sse-url', 'http://127.0.0.1:7800/v1/responses',
    '--mock-responses-websocket-url', 'ws://127.0.0.1:7802/v1/responses',
    '--anthropic-sse-url', 'http://127.0.0.1:7801/v1/messages',
    '--openai-model', 'gateway-openai-model',
    '--anthropic-model', 'gateway-anthropic-model',
  ];
  assert.throws(
    () => parseCliArgs([...base, '--api-key-env', 'AMBIGUOUS_KEY'], { AMBIGUOUS_KEY: 'secret' }),
    /invalid argument: --api-key-env/u,
  );
  assert.throws(
    () => parseCliArgs([
      ...base,
      '--openai-api-key-env', 'SHARED_KEY',
      '--anthropic-api-key-env', 'SHARED_KEY',
    ], { SHARED_KEY: 'secret' }),
    /must use distinct environment variables/u,
  );
  assert.throws(
    () => parseCliArgs([
      ...base,
      '--openai-api-key-env', 'OPENAI_KEY',
      '--anthropic-api-key-env', 'ANTHROPIC_KEY',
    ], { OPENAI_KEY: 'same-secret', ANTHROPIC_KEY: 'same-secret' }),
    /Application API keys must be distinct/u,
  );
});
