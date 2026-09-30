'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { buildMailboxEvidence } = require('../mailbox-evidence.cjs');

function fixture() {
  const meta = { threadId: 'root', mailboxTurnId: 'parent', startedUtc: '2026-09-30T00:00:00Z' };
  const events = [];
  const item = (atMs, kind) => events.push({ atMs, method: 'item/completed', params: { threadId: 'root',
    turnId: 'parent', item: { type: 'subAgentActivity', kind, agentThreadId: 'child', agentPath: '/root/child' } } });
  const log = (ms, target, message, span = 'responses_websocket.stream_request{}') => events.push({
    atMs: ms, kind: 'client/stderr', text: `${new Date(Date.parse(meta.startedUtc) + ms).toISOString()} INFO session_loop{thread_id=root}:turn{thread.id=root turn.id=parent}:${span}: ${target}: ${message}\n` });
  item(10, 'started');
  log(100, 'codex_api::endpoint::responses_websocket', 'new');
  log(150, 'codex_otel.agent_communication', 'communication_id=comm kind=result state=send sender_thread_id=child receiver_thread_id=root');
  log(160, 'codex_otel.agent_communication', 'communication_id=comm state=receive');
  events.push({ atMs: 170, method: 'item/completed', params: { threadId: 'root', turnId: 'parent', item: { type: 'reasoning' } } });
  log(180, 'codex_api::endpoint::responses_websocket', 'close time.busy=1ms');
  log(185, 'codex_core::session::turn', 'post sampling token usage model_needs_follow_up=true');
  log(190, 'codex_api::endpoint::responses_websocket', 'new');
  log(200, 'codex_api::endpoint::responses_websocket', 'successfully connected to websocket: ws://127.0.0.1:7600/v1/responses', 'responses_websocket.connect{}');
  log(300, 'codex_api::endpoint::responses_websocket', 'close time.busy=1ms');
  item(310, 'completed');
  events.push({ atMs: 400, method: 'turn/completed', params: { threadId: 'root', turn: { id: 'parent', status: 'completed' } } });
  const call = (id, flowRunId, ms, input) => ({ id, flowRunId, eventType: 'provider_semantic_step',
    createdAt: new Date(Date.parse(meta.startedUtc) + ms).toISOString(), originalPayload: {
      kind: 'model_call', trigger_request_id: id, body: JSON.stringify({ native_request: {
        digest: 'opaque-request-digest', wire_body: { input } } }) } });
  const original = [call('before-call', 'before', 110, []), call('after-call', 'after', 220,
    [{ type: 'agent_message', author: '/root/child', recipient: '/root', content: [{ type: 'input_text',
      text: 'Message Type: FINAL_ANSWER\nTask name: /root\nSender: /root/child\nPayload:\nSource finding.' }] }])];
  const flows = ['before', 'after'].map(flowRunId => ({ flowRunId, clientThreadId: 'root', logContext: { turn_id: 'parent' } }));
  return { events, meta, flows, original };
}

test('joins actual stream, communication, preemption trace and Native mailbox request', () => {
  const result = buildMailboxEvidence(fixture());
  assert.equal(result.status, 'COLLECTED');
  assert.deepEqual(result.records.map(r => r.kind), ['parent/native-sampling-before',
    'mailbox/child-completed', 'gateway/websocket-reconnected', 'parent/native-sampling-after']);
  assert.equal(result.records[2].originalPayload.payload.mailbox_input_present, true);
  assert.ok(result.records[2].clientCarrierSourceEventId.startsWith('timeline:'));
});

test('joins official span records split across stderr chunks', () => {
  const f = fixture();
  f.events = f.events.flatMap(e => e.kind === 'client/stderr' ? [
    { ...e, text: e.text.slice(0, 31) }, { ...e, text: e.text.slice(31) },
  ] : [e]);
  assert.equal(buildMailboxEvidence(f).status, 'COLLECTED');
});

test('excludes child prewarm inherited from the parent spawn span', () => {
  const f = fixture();
  const start = f.events.find(e => e.text?.includes(': new'));
  const close = f.events.find(e => e.text?.includes(': close'));
  const prewarm = [start, close].map((e, i) => ({ ...e, atMs: 120 + i * 20,
    text: e.text.replace(/2026-09-30T00:00:00.\d{3}Z/, new Date(Date.parse(f.meta.startedUtc) + 120 + i * 20).toISOString())
      .replace('responses_websocket.stream_request{', 'thread_spawn{agent_id=child}:responses_websocket.stream_request{') }));
  f.events.push(...prewarm);
  f.events.sort((a, b) => a.atMs - b.atMs);
  assert.equal(buildMailboxEvidence(f).status, 'COLLECTED');
});

for (const [name, change] of [
  ['ordinary network disconnect', f => { f.events = f.events.filter(e => !e.text?.includes('post sampling token usage')); }],
  ['completion with a tool call, not mailbox preemption', f => { f.events.push({ atMs: 165, method: 'item/started', params: { threadId: 'root', turnId: 'parent', item: { type: 'commandExecution' } } }); }],
  ['missing linked result sender', f => { f.events = f.events.filter(e => !e.text?.includes('state=send')); }],
  ['wrong result recipient', f => { f.events.find(e => e.text?.includes('state=send')).text = f.events.find(e => e.text?.includes('state=send')).text.replace('receiver_thread_id=root', 'receiver_thread_id=other'); }],
  ['missing actual WS connection', f => { f.events = f.events.filter(e => !e.text?.includes('successfully connected')); }],
  ['no mailbox notification in request', f => { const b = JSON.parse(f.original[1].originalPayload.body); b.native_request.wire_body.input = []; f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['unrelated request turn', f => { f.flows[1].logContext.turn_id = 'other'; }],
  ['client retry instead of successful preemption', f => { f.events.push({ atMs: 175, method: 'error', params: { threadId: 'root', turnId: 'parent', willRetry: true } }); }],
  ['nonexistent stream span name', f => { f.events = f.events.map(e => e.text ? { ...e,
    text: e.text.replaceAll('responses_websocket.stream_request{', 'responses.stream_ws_request{') } : e); }],
  ['unrelated thread mentioning the root identifier', f => { f.events = f.events.map(e => e.text ? { ...e,
    text: e.text.replace('thread.id=root', 'thread.id=other root_reference=root') } : e); }],
  ['unrelated turn mentioning the parent identifier', f => { f.events = f.events.map(e => e.text ? { ...e,
    text: e.text.replace('turn.id=parent', 'turn.id=other parent_reference=parent') } : e); }],
  ['completion from a different agent path', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].content[0].text = b.native_request.wire_body.input[0].content[0].text.replace('Sender: /root/child', 'Sender: /root/other');
    f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['completion addressed to another parent', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].content[0].text = b.native_request.wire_body.input[0].content[0].text.replace('Task name: /root', 'Task name: /other');
    f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['completion header quoted in ordinary payload', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].content[0].text = 'Quoted source:\n' + b.native_request.wire_body.input[0].content[0].text;
    f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['user prose posing as an agent completion', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].type = 'message'; b.native_request.wire_body.input[0].role = 'user';
    f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['different structured author', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].author = '/root/other'; f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['different structured recipient', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].recipient = '/other'; f.original[1].originalPayload.body = JSON.stringify(b); }],
  ['encrypted completion cannot prove plaintext identity', f => { const b = JSON.parse(f.original[1].originalPayload.body);
    b.native_request.wire_body.input[0].content.push({ type: 'encrypted_content', encrypted_content: 'opaque' });
    f.original[1].originalPayload.body = JSON.stringify(b); }],
]) test(`rejects ${name}`, () => {
  const f = fixture();
  change(f);
  const result = buildMailboxEvidence(f);
  assert.equal(result.status, 'UNVERIFIED');
  assert.deepEqual(result.records, []);
});
