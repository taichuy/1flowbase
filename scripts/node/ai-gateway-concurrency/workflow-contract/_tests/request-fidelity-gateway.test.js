'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { REQUEST_FIDELITY_VECTORS } = require('../../protocol-oracle/request-fidelity');
const { requestPair } = require('../request-fidelity-gateway');

test('Root #1477 AC-001/005: live request pairs target protocol-matched providers', () => {
  const target = (code) => ({
    model: '1flowbase', upstream_model: `${code}-upstream-model`, api_key: `${code}-key`,
    gateway: {
      responses_url: 'http://127.0.0.1:7800/v1/responses',
      chat_completions_url: 'http://127.0.0.1:7800/v1/chat/completions',
      anthropic_messages_url: 'http://127.0.0.1:7800/v1/messages',
    },
  });
  const ready = { targets: {
    openai: target('openai'),
    openai_compatible: target('openai-compatible'),
    anthropic: target('anthropic'),
  } };
  const rows = Object.fromEntries(REQUEST_FIDELITY_VECTORS.map((vector) => [
    vector.ingress, requestPair(vector, ready, 'http://127.0.0.1:9000'),
  ]));
  assert.match(rows.openai_chat.gatewayUrl, /\/v1\/chat\/completions/u);
  assert.match(rows.openai_responses.gatewayUrl, /\/v1\/responses/u);
  assert.match(rows.anthropic_messages.gatewayUrl, /\/v1\/messages/u);
  assert.equal(rows.openai_chat.gatewayBody.model, '1flowbase');
  assert.equal(rows.openai_chat.directBody.model, 'openai-compatible-upstream-model');
  assert.equal(rows.anthropic_messages.gatewayBody.model, '1flowbase');
  assert.equal(rows.anthropic_messages.directBody.model, 'anthropic-upstream-model');
  assert.equal(rows.openai_responses.gatewayBody.model, '1flowbase');
  assert.equal(rows.openai_responses.directBody.model, 'openai-upstream-model');
  assert.deepEqual(rows.openai_chat.directBody.stream_options, { include_usage: true });
  assert.equal(rows.openai_chat.directBody.max_tokens, 4096);
  assert.equal(rows.openai_chat.gatewayBody.max_tokens, 4096);
  assert.equal(rows.anthropic_messages.directBody.messages[0].content[0].text, 'Root #1477 request fidelity probe');
  assert.equal(rows.openai_responses.gatewayBody.input, 'Root #1477 request fidelity probe');
  assert.equal(rows.openai_responses.directBody.input, rows.openai_responses.gatewayBody.input);
  assert.equal(rows.openai_responses.directBody.max_output_tokens, 4096);
  assert.equal(rows.openai_responses.gatewayBody.max_output_tokens, 4096);
});
