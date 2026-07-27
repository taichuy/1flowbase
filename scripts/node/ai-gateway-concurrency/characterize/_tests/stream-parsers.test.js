'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const {
  eventText, isProtocolFailureTerminal, protocolErrorMessage, protocolEventType, protocolRunId,
} = require('../stream-parsers');

test('AC durable: protocol run ids come from Responses and Anthropic initial SSE events', () => {
  assert.equal(
    protocolRunId('responses-sse', { data: { type: 'response.created', response: { id: 'resp_018f7af7-3694-7ba0-90bf-83b5ec689705' } } }),
    'resp_018f7af7-3694-7ba0-90bf-83b5ec689705',
  );
  assert.equal(
    protocolRunId('anthropic-sse', { data: { type: 'message_start', message: { id: 'msg_018f7af7-3694-7ba0-90bf-83b5ec689705' } } }),
    'msg_018f7af7-3694-7ba0-90bf-83b5ec689705',
  );
  assert.equal(
    protocolRunId('chat-completions-sse', { data: { id: 'chatcmpl-018f7af7-3694-7ba0-90bf-83b5ec689705' } }),
    'chatcmpl-018f7af7-3694-7ba0-90bf-83b5ec689705',
  );
});

test('AC error fidelity: protocol error messages preserve the decoded string verbatim', () => {
  const message = ' {"future_error":{"shape":"unknown"}}\n ';
  assert.equal(protocolErrorMessage({ data: { error: { message } } }), message);
  assert.equal(protocolErrorMessage({ data: { response: { error: { message } } } }), message);
  assert.equal(protocolErrorMessage({ data: { error: { type: 'missing-message' } } }), null);
});

test('AC-002: Chat chunks expose text and one synthetic terminal type', () => {
  const delta = { data: { choices: [{ delta: { content: '中文🙂' }, finish_reason: null }] } };
  const terminal = { data: { choices: [{ delta: {}, finish_reason: 'stop' }] } };
  assert.equal(protocolEventType(delta), 'chat.completion.chunk');
  assert.equal(eventText(delta), '中文🙂');
  assert.equal(protocolEventType(terminal), 'chat.completion.done');
});

test('AC-004: explicit Responses and Anthropic failure events are terminal failures', () => {
  assert.equal(isProtocolFailureTerminal('responses-sse', 'response.failed'), true);
  assert.equal(isProtocolFailureTerminal('anthropic-sse', 'error'), true);
  assert.equal(isProtocolFailureTerminal('chat-completions-sse', 'error'), true);
  assert.equal(isProtocolFailureTerminal('responses-sse', 'response.completed'), false);
  assert.equal(isProtocolFailureTerminal('anthropic-sse', 'message_stop'), false);
  assert.equal(isProtocolFailureTerminal('responses-websocket', 'error'), false);
});
