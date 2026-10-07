'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { collect } = require('../collector');
const codex = require('../adapters/codex');
const { parse } = require('../../agent-logs-collector');
const timestamp = '2026-10-07T08:00:00.123Z';
const row = (type, payload, extra = {}) => ({ timestamp, type, payload, ...extra });
const meta = (id = 'session-a', extra = {}) => row('session_meta', { id, model_provider: 'local-provider', ...extra });
const turn = id => row('turn_context', { turn_id: id, model: 'model-exact' });
const message = (role, content, phase) => row('response_item', { type: 'message', role, content: [{ type: role === 'user' ? 'input_text' : 'output_text', text: content }], ...(phase ? { phase } : {}) });
const encode = rows => rows.map(value => JSON.stringify(value)).join('\n') + '\n';
async function fixture(t) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'agent-logs-fixture-'));
  const source = path.join(dir, 'source'); await fs.mkdir(source);
  const received = [], accepted = new Map(); let fail = false, incomplete = false, receiptShape = value => ({ data: value, meta: null });
  const server = http.createServer(async (request, response) => {
    let body = ''; for await (const chunk of request) body += chunk;
    const batch = JSON.parse(body); received.push({ batch, authorization: request.headers.authorization });
    if (fail) { response.writeHead(503).end('{}'); return; }
    let fresh = 0, duplicate = 0;
    for (const event of batch.events) {
      const identity = `${batch.source_id}:${event.event_id}`;
      if (accepted.has(identity)) {
        if (accepted.get(identity) !== JSON.stringify(event)) { response.writeHead(409).end('{}'); return; }
        duplicate++;
      } else { accepted.set(identity, JSON.stringify(event)); fresh++; }
    }
    response.setHeader('Content-Type', 'application/json');
    response.end(JSON.stringify(receiptShape({ accepted_events: incomplete ? 0 : fresh, duplicate_events: incomplete ? 0 : duplicate, record_ids: ['record-a'] })));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { await new Promise(resolve => server.close(resolve)); await fs.rm(dir, { recursive: true, force: true }); });
  return { dir, source, received, accepted, setFail: value => { fail = value; }, setIncomplete: value => { incomplete = value; }, setReceiptShape: value => { receiptShape = value; },
    options: { endpoint: `http://127.0.0.1:${server.address().port}/api/logs/v1/events`, key: 'fixture-secret', source,
      state: path.join(dir, 'state.json'), sourceId: 'fixture-installation', batchSize: 2 },
  };
}
test('canonical ApiSuccess HTTP receipt supports complete lines, resume and archive identity', async t => {
  const f = await fixture(t); const file = path.join(f.source, 'rollout-a.jsonl');
  const partial = JSON.stringify(message('assistant', 'final result', 'final_answer'));
  await fs.writeFile(file, encode([meta(), turn('turn-a'), message('user', 'question'), message('assistant', 'thinking', 'commentary')]) + partial.slice(0, 30));
  assert.equal((await collect(f.options)).uploaded, 4);
  const before = f.received.flatMap(r => r.batch.events);
  assert.ok(before.every(event => event.occurred_at === timestamp));
  assert.equal(before.find(event => event.content === 'thinking').phase, 'commentary');
  assert.equal(before.find(event => event.kind === 'user').source_task_id, 'turn-a');
  assert.equal((await collect(f.options)).uploaded, 0);
  await fs.appendFile(file, partial.slice(30) + '\n');
  assert.equal((await collect(f.options)).uploaded, 1);
  const final = f.received.at(-1).batch.events[0]; assert.equal(final.phase, 'final_answer');
  await fs.mkdir(path.join(f.source, 'archived_sessions')); await fs.rename(file, path.join(f.source, 'archived_sessions', 'rollout-a.jsonl'));
  assert.equal((await collect(f.options)).uploaded, 0);
  const replay = { ...f.options, state: path.join(f.dir, 'replay.json') };
  assert.equal((await collect(replay)).uploaded, 5);
  assert.equal(f.accepted.size, 5);
  assert.ok(f.received.every(r => r.authorization === 'Bearer fixture-secret'));
});
test('failed upload and incomplete wrapped ACK never advance checkpoint; retries replay safely', async t => {
  const f = await fixture(t); await fs.writeFile(path.join(f.source, 'rollout.jsonl'), encode([meta(), turn('turn-a'), message('user', 'question')]));
  f.setFail(true); await assert.rejects(collect(f.options), /503/);
  assert.deepEqual(JSON.parse(await fs.readFile(f.options.state, 'utf8')).files, {});
  f.setFail(false); f.setIncomplete(true); await assert.rejects(collect(f.options), /Incomplete/);
  assert.deepEqual(JSON.parse(await fs.readFile(f.options.state, 'utf8')).files, {});
  f.setIncomplete(false); assert.equal((await collect(f.options)).uploaded, 3);
  assert.equal(f.accepted.size, 3);
});
test('acknowledged source mutation and truncation are rejected', async t => {
  const f = await fixture(t); const file = path.join(f.source, 'a.jsonl');
  const rows = [meta(), turn('turn-a'), message('user', 'question')]; await fs.writeFile(file, encode(rows));
  await collect(f.options);
  await fs.writeFile(file, encode([meta(), turn('turn-a'), message('user', 'mutation')]));
  await assert.rejects(collect(f.options), /prefix changed/);
  await fs.writeFile(file, encode([meta()])); await assert.rejects(collect(f.options), /truncated/);
});
test('all recursively selected files are collected, same text at separate positions remains separate', async t => {
  const f = await fixture(t); await fs.mkdir(path.join(f.source, 'sessions', 'nested'), { recursive: true });
  await fs.mkdir(path.join(f.source, 'archived_sessions'));
  await fs.writeFile(path.join(f.source, 'sessions', 'nested', 'a.jsonl'), encode([meta('a'), turn('a'), message('user', 'same'), message('user', 'same')]));
  await fs.writeFile(path.join(f.source, 'archived_sessions', 'b.jsonl'), encode([meta('b'), turn('b'), message('user', 'same')]));
  await fs.writeFile(path.join(f.source, 'ignored.txt'), 'not a rollout');
  assert.equal((await collect(f.options)).uploaded, 7);
  assert.equal(f.received.flatMap(r => r.batch.events).filter(event => event.kind === 'user').length, 3);
});
test('typed response delta, old cumulative observations, inherited and root turn facts stay distinct', () => {
  const context = codex.createContext(meta('child', { parent_thread_id: 'parent', session_id: 'root-session', subagent_history_start_ordinal: 4,
    history_base: { thread_id: 'prefix-rollout', end_ordinal_exclusive: 4, end_byte_offset: 1024 } }));
  const convert = (line, start = 0) => codex.convert(line, context, { start });
  const inherited = convert({ ...message('user', 'inherited'), ordinal: 2, metadata: { inherited_user_message: true } });
  assert.equal(inherited.inherited, true); assert.equal(inherited.source_task_id, null);
  const parentContext = convert(row('turn_context', { turn_id: 'copied-parent-turn', model: 'parent-model' }, { ordinal: 3 }));
  assert.equal(parentContext.inherited, true); assert.equal(parentContext.source_task_id, null);
  const contextEvent = convert(row('turn_context', { turn_id: 'child-turn', root_turn_id: 'root-turn', model: 'model-exact' }, { ordinal: 4 }));
  assert.equal(contextEvent.parent_source_task_id, 'root-turn'); assert.equal(contextEvent.inherited, false);
  const typed = convert(row('token_usage_record', { thread_id: 'child', turn_id: 'child-turn', root_turn_id: 'root-turn', response_id: 'resp-a',
    usage: { input_tokens: 12, cached_input_tokens: 2, cache_write_input_tokens: 1, output_tokens: 4, total_tokens: 16 },
    turn_token_usage: { total_tokens: 80 }, thread_token_usage: { total_tokens: 500 } }));
  assert.deepEqual(typed.usage, { basis: 'delta', response_id: 'resp-a', input_tokens: 12, input_cache_hit_tokens: 2, cache_write_tokens: 1, output_tokens: 4, total_tokens: 16 });
  assert.equal(typed.provider_code, 'local-provider'); assert.equal(typed.model_id, 'model-exact');
  assert.equal(typed.source_session_id, 'child'); assert.equal(typed.raw.payload.thread_token_usage.total_tokens, 500);
  const old = convert(row('event_msg', { type: 'token_count', info: { total_token_usage: { total_tokens: 700 }, last_token_usage: { total_tokens: 200 } } }));
  assert.deepEqual(old.usage, { basis: 'cumulative', total_tokens: 700 });
  assert.equal(convert(message('assistant', 'no phase')).phase, null);
  assert.equal(convert(row('event_msg', { type: 'agent_message', message: 'mirror', phase: 'final_answer' })).kind, 'context');
  assert.equal(convert(row('compacted', { message: 'summary', replacement_history: [] })).kind, 'context');
  assert.equal(convert(row('event_msg', { type: 'task_complete', last_agent_message: 'not fabricated final' })).content, null);
  const unknown = codex.createContext(meta('unknown', { model_provider: null }));
  assert.equal(codex.convert(message('assistant', 'x'), unknown, { start: 10 }).provider_code, null);
});
test('explicit metadata turn IDs are used and unowned history never fabricates turns', () => {
  const context = codex.createContext(meta());
  const first = codex.convert(message('user', 'a'), context, { start: 100 });
  const second = codex.convert(message('user', 'a'), context, { start: 200 });
  assert.equal(first.source_task_id, null); assert.equal(second.source_task_id, null);
  const modern = message('assistant', 'answer', 'final_answer'); modern.payload.internal_chat_message_metadata_passthrough = { turn_id: 'source-turn' };
  assert.equal(codex.convert(modern, context, { start: 300 }).source_task_id, 'source-turn');
});
test('trusted local adapter uses shared scheduler and transport', async t => {
  const f = await fixture(t); const adapter = path.join(f.dir, 'adapter.cjs');
  await fs.writeFile(adapter, `module.exports={sourceClient:'fixture-client',createContext:()=>({}),convert:line=>({source_session_id:'fixture',source_task_id:'fixture-turn',parent_source_task_id:null,occurred_at:line.timestamp,kind:'context',content:null,phase:null,name:null,call_id:null,model_id:null,provider_code:null,usage:null,inherited:false,raw:line})}`);
  await fs.writeFile(path.join(f.source, 'a.jsonl'), encode([row('fixture', { value: 1 })]));
  assert.equal((await collect({ ...f.options, adapter })).uploaded, 1);
  assert.equal(f.received[0].batch.source_client, 'fixture-client');
});
test('CLI accepts explicit paths and refuses credential-bearing endpoints', () => {
  const options = parse(['import', '--endpoint', 'https://example.test/api/logs/v1/events', '--source', '/selected', '--source-id', 'stable']);
  assert.equal(options.sourceId, 'stable'); assert.equal(options.adapter, 'codex');
  assert.throws(() => parse(['import', '--endpoint', 'https://key@example.test/', '--source', '/selected']), /credentials/);
  assert.throws(() => parse(['watch', '--endpoint', 'https://example.test/', '--source', '/selected', '--batch-size', '0']), /positive/);
});

test('session-only facts wait without creating tasks or advancing checkpoint; later explicit turn owns them', async t => {
  const f = await fixture(t); const file = path.join(f.source, 'a.jsonl');
  await fs.writeFile(file, encode([meta(), row('compacted', { message: 'context' })]));
  const first = await collect(f.options); assert.equal(first.uploaded, 0); assert.equal(first.unattributed_files, 1);
  assert.deepEqual(JSON.parse(await fs.readFile(f.options.state, 'utf8')).files, {});
  await fs.appendFile(file, encode([turn('reliable-turn'), message('user', 'question')]));
  assert.equal((await collect(f.options)).uploaded, 4);
  assert.ok(f.received.flatMap(r => r.batch.events).every(event => event.source_task_id === 'reliable-turn'));
});

test('copied inherited turns are attached to the first child-local turn, never promoted into child tasks', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.source, 'child.jsonl'), encode([
    meta('child', { subagent_history_start_ordinal: 4 }),
    row('turn_context', { turn_id: 'parent-turn', root_turn_id: 'root-turn', model: 'parent-model' }, { ordinal: 1 }),
    { ...message('user', 'copied question'), ordinal: 2 },
    row('token_usage_record', { turn_id: 'parent-turn', root_turn_id: 'root-turn', usage: { total_tokens: 99 } }, { ordinal: 3 }),
    row('turn_context', { turn_id: 'child-turn', root_turn_id: 'root-turn', model: 'child-model' }, { ordinal: 4 }),
    { ...message('user', 'child question'), ordinal: 5 }
  ]));
  assert.equal((await collect(f.options)).uploaded, 6);
  const events = f.received.flatMap(r => r.batch.events);
  assert.ok(events.every(event => event.source_task_id === 'child-turn'));
  assert.equal(events.filter(event => event.inherited).length, 3);
  assert.equal(events.find(event => event.content === 'child question').model_id, 'child-model');
});

test('only JSON.data is accepted; top-level, absent data and invalid wrapped ACK retain checkpoint', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.source, 'ack.jsonl'), encode([meta(), turn('turn-ack')]));
  const invalidShapes = [
    value => value, // Counterexample: the old invented top-level fixture.
    () => ({ meta: null }),
    () => ({ data: null, meta: null }),
    value => ({ data: { ...value, record_ids: [123] }, meta: null }),
    value => ({ data: { ...value, accepted_events: -1 }, meta: null }),
    value => ({ data: { ...value, duplicate_events: 0.5 }, meta: null }),
    value => ({ data: { ...value, accepted_events: 0, duplicate_events: 0 }, meta: null }),
  ];
  for (const shape of invalidShapes) {
    f.setReceiptShape(shape);
    await assert.rejects(collect(f.options), /Incomplete upload receipt/);
    assert.deepEqual(JSON.parse(await fs.readFile(f.options.state, 'utf8')).files, {});
  }
  f.setReceiptShape(value => ({ data: value, meta: null }));
  assert.equal((await collect(f.options)).uploaded, 2);
  assert.equal(f.accepted.size, 2); // Committed invalid receipts replay as duplicates.
  assert.equal((await collect(f.options)).uploaded, 0);
});

test('cancelled task end is source-neutral, retains raw and does not manufacture final output', async t => {
  const f = await fixture(t);
  const aborted = row('event_msg', { type: 'turn_aborted', turn_id: 'cancelled-turn', reason: 'interrupted', detail: { source: 'fixture' } });
  await fs.writeFile(path.join(f.source, 'cancelled.jsonl'), encode([meta(), turn('cancelled-turn'), aborted]));
  assert.equal((await collect(f.options)).uploaded, 3);
  const end = f.received.flatMap(value => value.batch.events).find(event => event.kind === 'task_end');
  assert.equal(end.phase, 'cancelled'); assert.equal(end.content, null);
  assert.equal(end.source_task_id, 'cancelled-turn'); assert.deepEqual(end.raw, aborted);
  const context = codex.createContext(meta());
  codex.convert(turn('cancelled-turn'), context, { start: 1 });
  codex.convert(aborted, context, { start: 2 });
  assert.equal(context.turn, null);
});

test('transport preserves response delta plus unattributed historical cumulative as separate observations', async t => {
  const f = await fixture(t);
  const historical = row('event_msg', { type: 'token_count', info: { total_token_usage: { input_tokens: 600, output_tokens: 100, total_tokens: 700 }, last_token_usage: { total_tokens: 200 } } });
  await fs.writeFile(path.join(f.source, 'usage.jsonl'), encode([meta(), turn('usage-turn'),
    row('token_usage_record', { turn_id: 'usage-turn', response_id: 'response-R', usage: { input_tokens: 12, output_tokens: 4, total_tokens: 16 }, thread_token_usage: { total_tokens: 700 } }), historical]));
  assert.equal((await collect(f.options)).uploaded, 4);
  const events = f.received.flatMap(value => value.batch.events).filter(event => event.kind === 'usage');
  assert.equal(events.length, 2);
  assert.deepEqual(events[0].usage, { basis: 'delta', response_id: 'response-R', input_tokens: 12, output_tokens: 4, total_tokens: 16 });
  assert.deepEqual(events[1].usage, { basis: 'cumulative', input_tokens: 600, output_tokens: 100, total_tokens: 700 });
  assert.equal(Object.hasOwn(events[1].usage, 'response_id'), false);
  assert.deepEqual(events[1].raw, historical);
});
