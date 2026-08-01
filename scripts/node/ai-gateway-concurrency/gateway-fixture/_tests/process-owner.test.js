'use strict';

const assert = require('node:assert/strict');
const net = require('node:net');
const test = require('node:test');

const {
  DIAGNOSTIC_STREAM_LIMIT,
  OwnedProcessHealthTimeoutError,
  assertLoopbackPortAvailable,
  waitForHealth,
} = require('../process-owner');

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

test('owned child startup exit exposes typed bounded stdout and stderr diagnostics', async () => {
  const processHandle = {
    child: { exitCode: 17, signalCode: null },
    stdout: () => `prefix-${'x'.repeat(DIAGNOSTIC_STREAM_LIMIT + 100)}`,
    stderr: () => `stderr-${'y'.repeat(DIAGNOSTIC_STREAM_LIMIT + 200)}`,
  };
  await assert.rejects(
    waitForHealth('http://127.0.0.1:41731', 'api-server', { processHandle }),
    (error) => {
      assert.equal(error.code, 'owned_service_startup_exit');
      assert.equal(error.diagnostic.service, 'api-server');
      assert.equal(error.diagnostic.exit_code, 17);
      assert.equal(Buffer.byteLength(error.diagnostic.stdout.text) <= DIAGNOSTIC_STREAM_LIMIT, true);
      assert.equal(Buffer.byteLength(error.diagnostic.stderr.text) <= DIAGNOSTIC_STREAM_LIMIT, true);
      assert.equal(error.diagnostic.stdout.truncated_bytes > 0, true);
      assert.equal(error.diagnostic.stderr.truncated_bytes > 0, true);
      return true;
    },
  );
});

test('owned child health timeout exposes typed bounded stdout and stderr diagnostics', async () => {
  const stdout = `stdout-prefix-${'x'.repeat(DIAGNOSTIC_STREAM_LIMIT + 100)}`;
  const stderr = `stderr-prefix-${'y'.repeat(DIAGNOSTIC_STREAM_LIMIT + 200)}`;
  const processHandle = {
    child: { exitCode: null, signalCode: null },
    stdout: () => stdout,
    stderr: () => stderr,
  };
  let fetchCalls = 0;
  await assert.rejects(
    waitForHealth('http://127.0.0.1:41731', 'api-server', {
      processHandle,
      timeoutMs: 5,
      fetchImpl: async () => {
        fetchCalls += 1;
        return { ok: false };
      },
    }),
    (error) => {
      assert.equal(error instanceof OwnedProcessHealthTimeoutError, true);
      assert.equal(error.code, 'owned_service_health_timeout');
      assert.equal(error.message, 'api-server did not become healthy before timeout');
      assert.deepEqual(error.diagnostic, {
        service: 'api-server',
        exit_code: null,
        signal: null,
        stdout: {
          text: 'x'.repeat(DIAGNOSTIC_STREAM_LIMIT),
          truncated_bytes: Buffer.byteLength(stdout) - DIAGNOSTIC_STREAM_LIMIT,
        },
        stderr: {
          text: 'y'.repeat(DIAGNOSTIC_STREAM_LIMIT),
          truncated_bytes: Buffer.byteLength(stderr) - DIAGNOSTIC_STREAM_LIMIT,
        },
      });
      return true;
    },
  );
  assert.equal(fetchCalls, 1);
});
