'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { protocolRunId } = require('../stream-parsers');

test('AC durable: protocol run ids come from Responses and Anthropic initial SSE events', () => {
  assert.equal(
    protocolRunId('responses-sse', { data: { type: 'response.created', response: { id: 'resp_018f7af7-3694-7ba0-90bf-83b5ec689705' } } }),
    'resp_018f7af7-3694-7ba0-90bf-83b5ec689705',
  );
  assert.equal(
    protocolRunId('anthropic-sse', { data: { type: 'message_start', message: { id: 'msg_018f7af7-3694-7ba0-90bf-83b5ec689705' } } }),
    'msg_018f7af7-3694-7ba0-90bf-83b5ec689705',
  );
});
