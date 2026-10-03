'use strict';
const { snapshot, cpuDelta, sampledPeaks } = require('./proc.cjs');
const { outputOracle } = require('./payload.cjs');
function usageDelta(before, after) {
  return { user_cpu_seconds: (after.userCPUTime - before.userCPUTime) / 1e6, system_cpu_seconds: (after.systemCPUTime - before.systemCPUTime) / 1e6 };
}
async function measure(config, pid, hz, operation) {
  const observer = { user_cpu_seconds: 0, system_cpu_seconds: 0, wall_ms: 0, samples: 0 };
  const samples = [];
  const capture = () => {
    const start = performance.now();
    const before = process.resourceUsage();
    const sample = snapshot(pid, config.databasePid);
    const cost = usageDelta(before, process.resourceUsage());
    observer.user_cpu_seconds += cost.user_cpu_seconds;
    observer.system_cpu_seconds += cost.system_cpu_seconds;
    observer.wall_ms += performance.now() - start;
    observer.samples++;
    samples.push(sample);
    return sample;
  };
  const nodeBefore = process.resourceUsage();
  const start = performance.now();
  const before = capture();
  const timer = setInterval(capture, config.sampleMs);
  let value;
  try { value = await operation(); }
  finally { clearInterval(timer); }
  const after = capture();
  return {
    value, wall_seconds: (performance.now() - start) / 1000,
    cpu: cpuDelta(before, after, hz), sampled_peak: sampledPeaks(samples),
    samples, observer, harness_cpu: usageDelta(nodeBefore, process.resourceUsage()),
  };
}
async function request(target, body) {
  const started = performance.now();
  let ttft = null;
  const response = await fetch(target.responses_url, {
    method: 'POST', headers: { authorization: target.authorization, 'content-type': 'application/json' }, body,
    signal: AbortSignal.timeout(30000),
  });
  if (!response.ok || !response.headers.get('content-type')?.includes('text/event-stream')) { await response.body?.cancel(); throw Error('gateway response rejected'); }
  const decoder = new TextDecoder();
  let pending = '', bytes = 0;
  const events = [];
  function consume(final = false) {
    pending = pending.replace(/\r\n/g, '\n');
    let boundary;
    while ((boundary = pending.indexOf('\n\n')) >= 0) {
      const frame = pending.slice(0, boundary);
      pending = pending.slice(boundary + 2);
      const data = frame.split('\n').filter(line => line.startsWith('data:')).map(line => line.slice(5).trimStart()).join('\n');
      if (!data || data === '[DONE]') continue;
      const event = JSON.parse(data);
      if (events.length >= 2000) throw Error('gateway event limit');
      events.push(event);
      if (ttft === null && event.type === 'response.output_text.delta') ttft = performance.now() - started;
    }
    if (final && pending.trim()) throw Error('unterminated SSE frame');
  }
  for await (const chunk of response.body) {
    bytes += chunk.length;
    if (bytes > 2 * 1024 * 1024) throw Error('gateway response limit');
    pending += decoder.decode(chunk, { stream: true });
    consume();
  }
  pending += decoder.decode();
  consume(true);
  return { ...outputOracle(events), status: response.status, elapsed_ms: performance.now() - started, ttft_ms: ttft, request_bytes: Buffer.byteLength(body), response_bytes: bytes };
}
module.exports = { usageDelta, measure, request };
