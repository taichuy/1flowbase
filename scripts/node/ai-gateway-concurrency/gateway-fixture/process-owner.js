'use strict';

const net = require('node:net');
const { spawn } = require('node:child_process');
const { setTimeout: delay } = require('node:timers/promises');

const CAPTURE_LIMIT = 64 * 1024;
const DIAGNOSTIC_STREAM_LIMIT = 4 * 1024;

function boundedDiagnostic(value) {
  const encoded = Buffer.from(String(value || ''));
  if (encoded.length <= DIAGNOSTIC_STREAM_LIMIT) {
    return { text: encoded.toString('utf8'), truncated_bytes: 0 };
  }
  return {
    text: encoded.subarray(encoded.length - DIAGNOSTIC_STREAM_LIMIT).toString('utf8'),
    truncated_bytes: encoded.length - DIAGNOSTIC_STREAM_LIMIT,
  };
}

class OwnedProcessExitError extends Error {
  constructor(service, processHandle, status) {
    super(`${service} exited before becoming healthy (${status})`);
    this.name = 'OwnedProcessExitError';
    this.code = 'owned_service_startup_exit';
    this.diagnostic = {
      service,
      exit_code: processHandle.child?.exitCode ?? null,
      signal: processHandle.child?.signalCode ?? null,
      stdout: boundedDiagnostic(processHandle.stdout?.()),
      stderr: boundedDiagnostic(processHandle.stderr?.()),
    };
  }
}

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const port = server.address().port;
      server.close((error) => (error ? reject(error) : resolve(port)));
    });
  });
}

function assertLoopbackPortAvailable(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.once('error', (error) => {
      reject(new Error(`loopback port ${port} is unavailable: ${error.code || error.message}`));
    });
    server.listen(port, '127.0.0.1', () => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  });
}

function capture(stream) {
  let value = '';
  stream?.on('data', (chunk) => {
    value = `${value}${chunk}`.slice(-CAPTURE_LIMIT);
  });
  return () => value;
}

function spawnOwned(binary, env, options = {}, spawnImpl = spawn) {
  const child = spawnImpl(binary, [], {
    cwd: options.cwd || require('node:path').dirname(binary),
    env: { ...(options.parentEnv || process.env), ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const stdout = capture(child.stdout);
  const stderr = capture(child.stderr);
  return { child, stdout, stderr, output: () => `${stdout()}${stderr()}` };
}

function assertOwnedChildRunning(processHandle, service) {
  const child = processHandle?.child;
  const hasExitCode = child?.exitCode !== null && child?.exitCode !== undefined;
  const hasSignalCode = child?.signalCode !== null && child?.signalCode !== undefined;
  if (child && (hasExitCode || hasSignalCode)) {
    const status = hasExitCode
      ? `exit code ${child.exitCode}`
      : `signal ${child.signalCode}`;
    throw new OwnedProcessExitError(service, processHandle, status);
  }
}

async function waitForHealth(
  baseUrl,
  service,
  { fetchImpl = globalThis.fetch, timeoutMs = 30_000, processHandle = null } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    assertOwnedChildRunning(processHandle, service);
    try {
      const response = await fetchImpl(`${baseUrl}/health`, { signal: AbortSignal.timeout(1_000) });
      if (response.ok && (await response.json()).service === service) {
        assertOwnedChildRunning(processHandle, service);
        return;
      }
    } catch {
      // The owned child may still be binding its listener or migrating its database.
    }
    assertOwnedChildRunning(processHandle, service);
    await delay(100);
  }
  throw new Error(`${service} did not become healthy`);
}

async function stopOwned(processHandle, { delayImpl = delay } = {}) {
  const child = processHandle?.child;
  if (!child || child.exitCode !== null) return;
  child.kill('SIGTERM');
  await Promise.race([
    new Promise((resolve) => child.once('exit', resolve)),
    delayImpl(2_000),
  ]);
  if (child.exitCode === null) child.kill('SIGKILL');
}

module.exports = {
  DIAGNOSTIC_STREAM_LIMIT,
  OwnedProcessExitError,
  assertLoopbackPortAvailable,
  reserveLoopbackPort,
  spawnOwned,
  stopOwned,
  waitForHealth,
};
