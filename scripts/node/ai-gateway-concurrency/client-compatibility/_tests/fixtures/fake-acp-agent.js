'use strict';

const readline = require('node:readline');

const lines = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
let permissionRequestId = 900;
let pendingPrompt = null;
let promptCount = 0;

function send(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}

function update(value) {
  send({ jsonrpc: '2.0', method: 'session/update', params: { sessionId: 'fixture-session', update: value } });
}

lines.on('line', (line) => {
  const message = JSON.parse(line);
  if (message.method === 'initialize') {
    send({ jsonrpc: '2.0', id: message.id, result: {
      protocolVersion: 1,
      agentCapabilities: { loadSession: false },
      authMethods: [{ id: 'gateway', name: 'Gateway' }],
      agentInfo: { name: 'fixture-acp', version: '1' },
    } });
    return;
  }
  if (message.method === 'authenticate') {
    send({ jsonrpc: '2.0', id: message.id, result: {} });
    return;
  }
  if (message.method === 'session/new') {
    send({ jsonrpc: '2.0', id: message.id, result: { sessionId: 'fixture-session' } });
    return;
  }
  if (message.method === 'session/prompt') {
    promptCount += 1;
    update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: `marker-${promptCount}` } });
    if (promptCount === 1) {
      pendingPrompt = message.id;
      update({ sessionUpdate: 'tool_call', toolCallId: 'tool-1', title: 'Read fixture', status: 'pending' });
      send({ jsonrpc: '2.0', id: permissionRequestId, method: 'session/request_permission', params: {
        sessionId: 'fixture-session',
        toolCall: { toolCallId: 'tool-1', title: 'Read fixture', status: 'pending', content: [] },
        options: [{ optionId: 'allow', name: 'Allow', kind: 'allow_once' }],
      } });
      return;
    }
    send({ jsonrpc: '2.0', id: message.id, result: { stopReason: 'end_turn' } });
    return;
  }
  if (message.id === permissionRequestId && pendingPrompt !== null) {
    update({ sessionUpdate: 'tool_call_update', toolCallId: 'tool-1', status: 'completed' });
    update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'marker-after-tool' } });
    send({ jsonrpc: '2.0', id: pendingPrompt, result: { stopReason: 'end_turn' } });
    pendingPrompt = null;
  }
});
