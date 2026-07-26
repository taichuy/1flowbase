'use strict';

const assert = require('node:assert/strict');
const net = require('node:net');
const test = require('node:test');

const { assertLoopbackPortAvailable, waitForHealth } = require('../process-owner');

test('explicit loopback port availability rejects an occupied listener', async (context) => {
  const listener = net.createServer();
  await new Promise((resolve, reject) => {
    listener.once('error', reject);
    listener.listen(0, '127.0.0.1', resolve);
  });
  context.after(() => new Promise((resolve) => listener.close(resolve)));

  await assert.rejects(
    assertLoopbackPortAvailable(listener.address().port),
    /loopback port .* is unavailable: EADDRINUSE/u,
  );
});

test('health from an old listener cannot hide an exited owned child', async () => {
  const processHandle = { child: { exitCode: 1 } };
  let fetchCalls = 0;

  await assert.rejects(
    waitForHealth('http://127.0.0.1:7800', 'api-server', {
      processHandle,
      fetchImpl: async () => {
        fetchCalls += 1;
        return { ok: true, json: async () => ({ service: 'api-server' }) };
      },
    }),
    /api-server exited before becoming healthy \(exit code 1\)/u,
  );
  assert.equal(fetchCalls, 0);
});

test('owned child exit during a successful health response is still rejected', async () => {
  const processHandle = { child: { exitCode: null } };

  await assert.rejects(
    waitForHealth('http://127.0.0.1:7800', 'api-server', {
      processHandle,
      fetchImpl: async () => ({
        ok: true,
        async json() {
          processHandle.child.exitCode = 1;
          return { service: 'api-server' };
        },
      }),
    }),
    /api-server exited before becoming healthy \(exit code 1\)/u,
  );
});
