'use strict';

const { spawn } = require('node:child_process');

const MAX_OUTPUT_BYTES = 1024 * 1024;
const DEFAULT_TIMEOUT_MS = 180_000;
const SENTINEL_RESPONSE = '1flowbase gateway sentinel ok';

function boundedCollector(stream) {
  const chunks = [];
  let bytes = 0;
  let overflow = false;
  stream?.on('data', (chunk) => {
    bytes += chunk.length;
    if (bytes <= MAX_OUTPUT_BYTES) chunks.push(Buffer.from(chunk));
    else overflow = true;
  });
  return () => ({ text: Buffer.concat(chunks).toString('utf8'), bytes, overflow });
}

function executeInvocation(invocation, env, { spawnImpl = spawn, timeoutMs = DEFAULT_TIMEOUT_MS } = {}) {
  return new Promise((resolve, reject) => {
    const startedAt = new Date();
    const startedNs = process.hrtime.bigint();
    const child = spawnImpl(invocation.executable, invocation.args, {
      cwd: invocation.cwd,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const stdout = boundedCollector(child.stdout);
    const stderr = boundedCollector(child.stderr);
    let timedOut = false;
    const terminate = () => child.kill('SIGTERM');
    process.once('SIGINT', terminate);
    process.once('SIGTERM', terminate);
    const removeSignalHandlers = () => {
      process.removeListener('SIGINT', terminate);
      process.removeListener('SIGTERM', terminate);
    };
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill('SIGKILL');
    }, timeoutMs);
    child.once('error', (error) => {
      clearTimeout(timer);
      removeSignalHandlers();
      reject(error);
    });
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      removeSignalHandlers();
      resolve({
        started_at: startedAt.toISOString(),
        finished_at: new Date().toISOString(),
        duration_ms: Number(process.hrtime.bigint() - startedNs) / 1e6,
        exit_code: code,
        signal,
        timed_out: timedOut,
        stdout: stdout(),
        stderr: stderr(),
      });
    });
  });
}

function parseJsonLines(text, client) {
  const lines = text.split(/\r?\n/u).filter((line) => line.trim() !== '');
  if (lines.length === 0) throw new Error(`${client} emitted no JSONL events`);
  return lines.map((line) => {
    try {
      return JSON.parse(line);
    } catch {
      throw new Error(`${client} emitted invalid JSONL`);
    }
  });
}

function includesSentinel(value) {
  if (typeof value === 'string') return value.includes(SENTINEL_RESPONSE);
  if (Array.isArray(value)) return value.some(includesSentinel);
  if (value && typeof value === 'object') return Object.values(value).some(includesSentinel);
  return false;
}

function assertCompatibleResult(client, result) {
  if (result.timed_out) throw new Error(`${client} sentinel timed out`);
  if (result.exit_code !== 0) throw new Error(`${client} sentinel exited with ${result.exit_code}`);
  if (result.stdout.overflow || result.stderr.overflow) {
    throw new Error(`${client} sentinel output exceeded 1 MiB`);
  }
  const events = parseJsonLines(result.stdout.text, client);
  const compatible = client === 'codex'
    ? events.some((event) => event.type === 'item.completed'
      && event.item?.type === 'agent_message'
      && includesSentinel(event.item))
    : events.some((event) => (event.type === 'assistant' || event.type === 'result')
      && includesSentinel(event));
  if (!compatible) throw new Error(`${client} sentinel response marker was not observed`);
  return events.length;
}

module.exports = { SENTINEL_RESPONSE, assertCompatibleResult, executeInvocation, parseJsonLines };
