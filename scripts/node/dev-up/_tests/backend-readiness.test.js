const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { getServiceDefinitions } = require('../services.js');
const { probeHttpReadiness, waitForServiceReadiness } = require('../process.js');

function backend(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'backend-readiness-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  t.mock.timers.enable({ apis: ['Date'], now: 0 });
  return getServiceDefinitions(root)['api-server'];
}

function liveProcess(t) {
  return {
    readPidRecordImpl: () => ({ pid: 42 }),
    isProcessAliveImpl: () => true,
    sleepImpl: async (ms) => t.mock.timers.tick(ms),
  };
}

test('a stalled HTTP probe still times out independently of unlimited startup', async (t) => {
  const server = http.createServer(() => {});
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => {
    server.closeAllConnections();
    server.close();
  });
  const result = await probeHttpReadiness({
    probeHost: '127.0.0.1', port: server.address().port,
    startupTimeoutMs: null,
    readinessProbe: { path: '/health', timeoutMs: 30 },
  });
  assert.deepEqual(result, { ready: false, reason: 'HTTP GET /health timed out after 30ms' });
});

for (const key of ['api-server', 'web']) {
  test(`${key} can become ready after ten minutes without being terminated`, async (t) => {
    const service = backend(t);
    const selected = key === 'web' ? getServiceDefinitions(service.repoRoot).web : service;
    const result = await waitForServiceReadiness(selected, {
      ...liveProcess(t),
      probeHttpReadinessImpl: async () => ({ ready: Date.now() >= 600_000 }),
    });
    assert.equal(result.ready, true);
    assert.equal(Date.now(), 600_000);
  });
}

test('process exit replaces an earlier HTTP timeout and stops further probes', async (t) => {
  const service = backend(t);
  let probes = 0;
  const result = await waitForServiceReadiness(service, {
    ...liveProcess(t),
    isProcessAliveImpl: () => probes === 0,
    probeHttpReadinessImpl: async () => {
      probes += 1;
      return { ready: false, reason: 'HTTP timed out' };
    },
  });
  assert.equal(probes, 1);
  assert.deepEqual(result, { ready: false, reason: 'api-server process exited before readiness' });
});

test('cancelling an unready backend stops the readiness loop', async (t) => {
  const service = backend(t);
  const controller = new AbortController();
  service.signal = controller.signal;
  let probes = 0;
  await assert.rejects(waitForServiceReadiness(service, {
    ...liveProcess(t),
    probeHttpReadinessImpl: async () => {
      probes += 1;
      controller.abort();
      return { ready: false };
    },
  }), { name: 'AbortError' });
  assert.equal(probes, 1);
});

test('an explicit finite startup deadline remains supported', async (t) => {
  const service = backend(t);
  service.startupTimeoutMs = 30_000;
  service.startupDeadline = 30_000;
  const result = await waitForServiceReadiness(service, {
    ...liveProcess(t),
    probeHttpReadinessImpl: async () => ({ ready: false, reason: 'HTTP unavailable' }),
  });
  assert.deepEqual(result, { ready: false, reason: 'HTTP unavailable' });
  assert.equal(Date.now(), 30_000);
});
