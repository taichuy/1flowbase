'use strict';

const TRANSPORT = Object.freeze({
  RESPONSES_SSE: 'responses-sse',
  RESPONSES_WEBSOCKET: 'responses-websocket',
  ANTHROPIC_SSE: 'anthropic-sse',
});

const SCENARIO = Object.freeze({
  NORMAL: 'normal',
  SLOW: 'slow',
  CANCEL_OBSERVATION: 'cancel-observation',
  HTTP_500: 'http-500',
  STREAM_INTERRUPTION: 'stream-interruption',
});

const MOCK_ROUTE = Object.freeze({
  RESPONSES: '/v1/responses',
  CHAT_COMPLETIONS: '/v1/chat/completions',
  ANTHROPIC_MESSAGES: '/v1/messages',
});

const MOCK_SCENARIO_SENTINEL_PREFIX = '1flowbase-test-scenario=';

const SUCCESS_TERMINAL = Object.freeze({
  [TRANSPORT.RESPONSES_SSE]: 'response.completed',
  [TRANSPORT.RESPONSES_WEBSOCKET]: 'response.completed',
  [TRANSPORT.ANTHROPIC_SSE]: 'message_stop',
});

function assertScenario(scenario) {
  if (!Object.values(SCENARIO).includes(scenario)) {
    throw new Error(`unsupported mock scenario: ${scenario}`);
  }
  return scenario;
}

function mockScenarioSentinel(scenario) {
  return `[${MOCK_SCENARIO_SENTINEL_PREFIX}${assertScenario(scenario)}]`;
}

function assertTransport(transport) {
  if (!Object.values(TRANSPORT).includes(transport)) {
    throw new Error(`unsupported mock transport: ${transport}`);
  }
  return transport;
}

function assertDistinctRequestNonces(nonces) {
  const seen = new Set();
  for (const nonce of nonces) {
    if (typeof nonce !== 'string' || nonce.length === 0) {
      throw new Error('mock request nonce must be a non-empty string');
    }
    if (seen.has(nonce)) throw new Error(`mock request nonce was reused: ${nonce}`);
    seen.add(nonce);
  }
}

module.exports = {
  MOCK_ROUTE,
  MOCK_SCENARIO_SENTINEL_PREFIX,
  SCENARIO,
  SUCCESS_TERMINAL,
  TRANSPORT,
  assertDistinctRequestNonces,
  assertScenario,
  assertTransport,
  mockScenarioSentinel,
};
