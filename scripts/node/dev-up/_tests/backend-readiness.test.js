const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { getServiceDefinitions } = require('../services.js');
const { waitForServiceReadiness } = require('../process.js');

function backend(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'backend-readiness-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  t.mock.timers.enable({ apis: ['Date'], now: 0 });
  const service = getServiceDefinitions(root)['api-server'];
  service.startupDeadline = Date.now() + service.startupTimeoutMs;
  return service;
}

test('cold backend remains eligible for readiness after 30 seconds', async (t) => {
  const service = backend(t);
  const result = await waitForServiceReadiness(service, {
    probeHttpReadinessImpl: async () => ({ ready: Date.now() >= 45_000 }),
    sleepImpl: async (ms) => t.mock.timers.tick(ms),
  });
  assert.equal(result.ready, true);
  assert.equal(Date.now(), 45_000);
});

test('backend that never becomes healthy still exhausts its startup deadline', async (t) => {
  const service = backend(t);
  const result = await waitForServiceReadiness(service, {
    probeHttpReadinessImpl: async () => ({ ready: false, reason: 'HTTP unavailable' }),
    sleepImpl: async (ms) => t.mock.timers.tick(ms),
  });
  assert.deepEqual(result, { ready: false, reason: 'HTTP unavailable' });
  assert.equal(Date.now(), 120_000);
});
