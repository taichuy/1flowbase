'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const cp = require('node:child_process');
const crypto = require('node:crypto');
const { createMock } = require('./mock-upstream.cjs');
const { createOwnedGateway } = require('./gateway-owner.cjs');
const root = path.resolve(process.env.NRA_ARTIFACT_DIR || path.join(__dirname, '../../../../tmp/test-governance/native-recovery-fix-20261002/mock'));
const apiBinary = process.env.NRA_API_BINARY || path.resolve(__dirname, '../../../../api/target/debug/api-server');
fs.mkdirSync(root, { recursive: true });
function parseSse(raw) {
  return raw.split(/\r?\n\r?\n/).flatMap(block => {
    const data = block.split(/\r?\n/).filter(x => x.startsWith('data:')).map(x => x.slice(5).trim()).join('\n');
    if (!data || data === '[DONE]') return [];
    try { return [JSON.parse(data)]; } catch { return []; }
  });
}
(async () => {
  const mock = await createMock('gateway_chain');
  let gateway;
  const requests = [];
  try {
    console.log('Starting task-owned gateway and database');
    gateway = await createOwnedGateway({ upstreamBaseUrl: mock.baseUrl, artifactRoot: path.join(root, 'service-logs') });
    console.log(JSON.stringify({ stage: 'ready', pid: gateway.gatewayPid, origin: gateway.baseUrl,
      database: gateway.databaseName, model: gateway.target.model }));
    async function post(body, label) {
      const response = await fetch(gateway.target.gateway.responses_url, {
        method: 'POST', headers: { 'content-type': 'application/json', authorization: gateway.target.gateway.authorization },
        body: JSON.stringify(body), signal: AbortSignal.timeout(45_000) });
      const raw = await response.text();
      const record = { label, status: response.status, content_type: response.headers.get('content-type'),
        events: parseSse(raw), raw };
      requests.push(record);
      fs.writeFileSync(path.join(root, `gateway-${label}.json`), JSON.stringify(record, null, 2));
      console.log(JSON.stringify({ stage: label, status: record.status, types: record.events.map(x => x.type),
        errors: record.events.filter(x => x.type === 'response.failed').map(x => x.response?.error),
        body: response.status >= 400 ? raw.slice(0, 1000) : undefined }));
      return record;
    }
    const body = { model: 'gpt-6-luna', reasoning: { effort: 'max' }, stream: true, store: false,
      instructions: 'Controlled fixture only. Tool calls are never executed.',
      input: [{ role: 'user', content: [{ type: 'input_text', text: 'Return one fixture tool call.' }] }],
      tools: [{ type: 'custom', name: 'exec', description: 'fixture only' }] };
    const first = await post(body, 'seed');
    assert.equal(first.status, 200);
    const completed = first.events.find(x => x.type === 'response.completed');
    assert.ok(completed, 'healthy seed must complete');
    const call = first.events.find(x => x.type === 'response.output_item.done' && x.item?.call_id)?.item;
    assert.ok(call, 'healthy seed must return formal tool item');
    const followup = { ...body, previous_response_id: completed.response.id,
      input: [{ type: 'custom_tool_call_output', call_id: call.call_id, output: 'fixture tool result' }] };
    const interrupted = await post(followup, 'interrupted');
    assert.equal(interrupted.status, 200);
    assert.ok(interrupted.events.some(x => x.type === 'response.failed'));
    assert.ok(!interrupted.events.some(x => x.type === 'response.completed'), 'partial tool call cannot complete');
    assert.ok(!interrupted.events.some(x => x.type === 'response.output_item.done' && x.item?.call_id), 'partial tool cannot be committed');
    const retry = await post(followup, 'retry');
    const q = `select jsonb_build_object('id',id,'status',status,'error',error_payload) from flow_runs where error_payload is not null order by started_at;`;
    const db = cp.execFileSync('docker', ['exec', 'docker-db-1', 'psql', '-U', 'postgres', '-d', gateway.databaseName, '-At', '-c', q], { encoding: 'utf8', maxBuffer: 1024 * 1024 });
    const flows = db.trim().split('\n').filter(Boolean).map(JSON.parse);
    const detailQ = `select jsonb_build_object('section',section,'payload',payload) from node_run_details where section in ('metrics_payload','error_payload');`;
    const details = cp.execFileSync('docker', ['exec', 'docker-db-1', 'psql', '-U', 'postgres', '-d', gateway.databaseName, '-At', '-c', detailQ], { encoding: 'utf8', maxBuffer: 2 * 1024 * 1024 });
    const evidence = { actual_api_binary: apiBinary,
      actual_api_binary_sha256: crypto.createHash('sha256').update(fs.readFileSync(apiBinary)).digest('hex'),
      model: 'gpt-6-luna', reasoning_effort: 'max', controlled_mock_only: true,
      requests, flows, details: details.trim().split('\n').filter(Boolean).map(JSON.parse), upstream: mock.trace };
    fs.writeFileSync(path.join(root, 'gateway-evidence.json'), JSON.stringify(evidence, null, 2));
    assert.equal(retry.status, 200, 'standard new request must not surface internal recovery 409');
    assert.ok(retry.events.some(x => x.type === 'response.completed'), 'new response must complete');
    assert.ok(!retry.events.some(x => x.type === 'response.failed'));
    assert.equal(mock.trace.filter(x => x.kind === 'request').length, 4,
      'two failed upstream attempts and one fresh response after the healthy seed');
    assert.ok(flows.some(x => x.error.error_code === 'provider_transport_unavailable'
      && x.error.ai_native_recovery.provider_inner_receipt), 'typed interruption receipt retained');
    assert.ok(!flows.some(x => x.error.error_code === 'provider_invalid_response'
      || x.error.ai_native_recovery.decision === 'missing_typed_receipt'));
    const accepted = cp.execFileSync('docker', ['exec','docker-db-1','psql','-U','postgres','-d',gateway.databaseName,'-At','-c',
      "select count(*) from flow_run_callback_resume_attempts;"], {encoding:'utf8',maxBuffer:4096}).trim();
    assert.equal(accepted, '1', 'already accepted callback must not be consumed twice');
    console.log('PASS: incomplete stream failed honestly; standard new response completed; tool receipt consumed once');
  } finally {
    await gateway?.close();
    await mock.close();
    console.log('Task-owned gateway, mock sockets, temporary session, database and role cleaned');
  }
})().catch(error => { console.error(error.stack); process.exitCode = 1; });
