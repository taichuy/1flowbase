#!/usr/bin/env node
'use strict';

// Paired runtime observation. Credentials never enter artifacts or command arguments.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

async function main() {
  const [label, countText, pidText] = process.argv.slice(2);
  const count = Number(countText), serverPid = Number(pidText);
  const rounds = Number(process.env.PROBE_ROUNDS || '1');
  if (!/^[a-z0-9-]+$/u.test(label || '') || !Number.isSafeInteger(count) || count < 1 || !Number.isSafeInteger(serverPid) || serverPid < 1) {
    throw new Error('Usage: TEST_KEY_FILE=... node worker-resource-probe.js LABEL CLIENT_COUNT SERVER_PID');
  }
  if (![1, 2].includes(rounds)) throw new Error('PROBE_ROUNDS must be 1 or 2');
  const repoRoot = path.resolve(__dirname, '../../..');
  const pnpmRoot = path.join(process.env.ONEFLOW_REPO || repoRoot, 'web/node_modules/.pnpm');
  const wsPackage = fs.readdirSync(pnpmRoot).filter(name => /^ws@8\./u.test(name)).sort().at(-1);
  if (!wsPackage) throw new Error('Installed ws@8 is required');
  const { WebSocket } = require(path.join(pnpmRoot, wsPackage, 'node_modules/ws'));
  const key = fs.readFileSync(process.env.TEST_KEY_FILE, 'utf8').trim();
  const endpoint = process.env.GATEWAY_RESPONSES_URL || 'ws://127.0.0.1:7600/v1/responses';
  const target = new URL(endpoint);
  if (!['ws:', 'wss:'].includes(target.protocol) || !['127.0.0.1', 'localhost'].includes(target.hostname)) throw new Error('Use an explicit loopback Gateway URL');
  const outputDir = path.join(repoRoot, 'tmp/test-governance/plugin-worker', label);
  fs.mkdirSync(outputDir, { recursive: true });
  const started = Date.now(), samples = [], sockets = [], rows = [];
  const now = () => Date.now() - started;
  function proc(pid) {
    try {
      const s = fs.readFileSync(`/proc/${pid}/status`, 'utf8');
      return { pid, rss_kib: Number(s.match(/^VmRSS:\s+(\d+)/mu)?.[1] || 0), parent: Number(s.match(/^PPid:\s+(\d+)/mu)?.[1]) };
    } catch { return null; }
  }
  function sample() {
    const childIds = (() => { try { return [...new Set(fs.readdirSync(`/proc/${serverPid}/task`).flatMap(tid => {
      try { return fs.readFileSync(`/proc/${serverPid}/task/${tid}/children`, 'utf8').trim().split(/\s+/u).filter(Boolean).map(Number); } catch { return []; }
    }))]; } catch { return []; } })();
    const workers = childIds.filter(pid => { try { return fs.readlinkSync(`/proc/${pid}/exe`).endsWith('/openai-provider'); } catch { return false; } }).map(proc).filter(Boolean);
    samples.push({ t_ms: now(), server: proc(serverPid), workers });
  }
  let release;
  const barrier = new Promise(resolve => { release = resolve; });
  let connected = 0;
  const timer = setInterval(sample, 100);
  sample();
  try {
    await Promise.all(Array.from({ length: count }, (_, index) => new Promise(resolve => {
      const token = crypto.randomBytes(12).toString('hex');
      const row = { index, rounds: [], errors: [] }; rows.push(row);
      const ws = new WebSocket(endpoint, { headers: { authorization: `Bearer ${key}`, 'openai-beta': 'responses_websockets=2026-02-06' } }); sockets.push(ws);
      let done = false, round;
      const deadline = setTimeout(() => finish('deadline'), 180000);
      function finish(error) {
        if (done) return; done = true;
        clearTimeout(deadline);
        if (error) row.errors.push(error);
        ws.close(); resolve();
      }
      function send(previous) {
        round = { sent_ms: now(), events: [], text: '', response_id: null };
        row.rounds.push(round);
        const request = { type: 'response.create', model: 'gpt-6-luna', reasoning: { effort: 'max' }, store: false,
          input: [{ role: 'user', content: [{ type: 'input_text', text: previous ? 'Repeat exactly the verification token I gave you in my preceding message, with no other text.' : `Return exactly this verification token with no other text: ${token}` }] }] };
        if (previous) request.previous_response_id = previous;
        ws.send(JSON.stringify(request));
      }
      ws.on('open', async () => {
        row.connected_ms = now(); connected += 1;
        if (connected === count) release();
        await barrier;
        if (!done) send();
      });
      ws.on('message', raw => {
        let frame;
        try { frame = JSON.parse(raw); } catch { return finish('invalid_json'); }
        if (!round || done) return;
        round.events.push({ type: frame.type, t_ms: now() });
        if (frame.type === 'response.output_text.delta') { round.text += frame.delta || ''; round.first_delta_ms ??= now(); }
        if (frame.response?.id) round.response_id = frame.response.id;
        if (frame.type === 'error' || frame.type === 'response.failed') {
          row.error_code = frame.error?.code || frame.response?.error?.code || 'unknown';
          row.error_message = String(frame.error?.message || frame.response?.error?.message || '').replaceAll(key, '[REDACTED]').slice(0, 400);
          return finish('response_error');
        }
        if (frame.type === 'response.completed') {
          round.completed_ms = now();
          const full = (frame.response?.output || []).flatMap(item => item.content || []).map(c => c.text || '').join('');
          round.token_matches = (full || round.text).trim() === token;
          delete round.text;
          if (!round.token_matches) return finish('content_mismatch');
          if (row.rounds.length < rounds && round.response_id) send(round.response_id); else finish();
        }
      });
      ws.on('error', () => finish('websocket_error'));
      ws.on('close', () => { if (!done) finish('unexpected_close'); });
    })));
  } finally {
    clearInterval(timer); sample();
    for (const ws of sockets) ws.terminate();
  }
  for (const row of rows) for (const round of row.rounds) delete round.text;
  const ids = rows.flatMap(row => row.rounds.map(round => round.response_id).filter(Boolean));
  const summary = { label, endpoint, count, rounds, model: 'gpt-6-luna', reasoning: 'max', elapsed_ms: now(),
    passed: rows.every(row => row.errors.length === 0 && row.rounds.length === rounds && row.rounds.every(round => round.token_matches)) && new Set(ids).size === ids.length,
    connected, completed_rounds: rows.flatMap(row => row.rounds).filter(round => round.completed_ms != null).length,
    initial_worker_count: samples[0]?.workers.length,
    peak_worker_count: Math.max(...samples.map(s => s.workers.length)),
    peak_worker_rss_kib: Math.max(...samples.map(s => s.workers.reduce((sum, worker) => sum + worker.rss_kib, 0))),
    note: 'Client timestamps show offered concurrency; actual worker overlap requires correlated Host/fixture evidence. RSS is sampled, not peak allocation or admission reservations.' };
  fs.writeFileSync(path.join(outputDir, 'result.json'), JSON.stringify({ summary, rows, samples }, null, 2));
  process.stdout.write(`${JSON.stringify(summary)}\n`);
  process.exitCode = summary.passed ? 0 : 1;
}

if (require.main === module) main().catch(() => { process.stderr.write('Probe failed; sensitive details withheld\n'); process.exitCode = 1; });
