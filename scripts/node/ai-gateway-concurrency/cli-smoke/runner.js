'use strict';

const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { appendTimelineEvent, readTimeline, writeMergedTimeline } = require('./timeline');

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

function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'\\''`)}'`;
}

function ptyMarkerTimeline(output, timing, markers) {
  const childOutput = output.replace(/^Script started on [^\r\n]*(?:\r?\n)/u, '');
  const outputBytes = Buffer.from(childOutput);
  let cursor = 0;
  let elapsedMs = 0;
  let visible = '';
  const observed = Object.fromEntries(markers.map((marker) => [marker, null]));
  for (const line of timing.split(/\r?\n/u)) {
    const match = /^O\s+([0-9.]+)\s+(\d+)$/u.exec(line.trim());
    if (!match) continue;
    elapsedMs += Number(match[1]) * 1000;
    const bytes = Number.parseInt(match[2], 10);
    visible += outputBytes.subarray(cursor, cursor + bytes).toString('utf8');
    cursor += bytes;
    for (const marker of markers) {
      if (observed[marker] === null && visible.includes(marker)) observed[marker] = elapsedMs;
    }
  }
  return observed;
}

function spawnResult(executable, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { stdio: ['ignore', 'pipe', 'pipe'], ...options });
    const stdout = boundedCollector(child.stdout);
    const stderr = boundedCollector(child.stderr);
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code, signal, stdout: stdout(), stderr: stderr() }));
  });
}

async function requireSuccess(executable, args, options) {
  const result = await spawnResult(executable, args, options);
  if (result.code !== 0) {
    throw new Error(`${path.basename(executable)} exited with ${result.code}: ${result.stderr.text.trim()}`);
  }
  return result;
}

async function executeTmuxInvocation(
  invocation,
  env,
  {
    artifactDirectory,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    tmuxExecutable = 'tmux',
    scriptExecutable = 'script',
    markers = [],
    clientResultMarker,
    onFirstMarker,
    producerTimelinePath,
    secrets = [],
  } = {}
) {
  if (!artifactDirectory) throw new Error('tmux invocation artifact directory is required');
  fs.mkdirSync(artifactDirectory, { recursive: true, mode: 0o700 });
  const root = fs.mkdtempSync(path.join(artifactDirectory, '.tmux-'));
  const ptyPath = path.join(root, 'pty.raw');
  const timingPath = path.join(root, 'timing.raw');
  const statusPath = path.join(root, 'status');
  const wrapperPath = path.join(root, 'run.sh');
  const secretsPath = path.join(root, 'secrets.json');
  const pipeDonePath = path.join(root, 'pipe.done');
  const timelinePath = path.join(artifactDirectory, 'timeline.jsonl');
  const pipeCapturePath = path.join(__dirname, 'pipe-pane-capture.js');
  const socket = `oneflowbase-stream-${process.pid}-${path.basename(root)}`;
  const session = 'compatible-stream';
  const doneSignal = `${socket}-done`;
  const releaseSignal = `${socket}-release`;
  const startSignal = `${socket}-start`;
  const command = [invocation.executable, ...invocation.args].map(shellQuote).join(' ');
  let markerWatcher = null;
  let firstMarkerReleased = false;
  let secondMarkerTerminationStarted = false;
  let barrierReleasePromise = null;
  const observedMarkers = new Set();
  fs.rmSync(timelinePath, { force: true });
  fs.writeFileSync(secretsPath, JSON.stringify(secrets), { mode: 0o600 });
  fs.writeFileSync(wrapperPath, [
    '#!/bin/sh',
    `${shellQuote(tmuxExecutable)} -L ${shellQuote(socket)} wait-for ${shellQuote(startSignal)}`,
    `${shellQuote(scriptExecutable)} -q -e -f -m advanced -O ${shellQuote(ptyPath)} -T ${shellQuote(timingPath)} -c ${shellQuote(command)}`,
    'status=$?',
    `printf '%s\\n' "$status" > ${shellQuote(statusPath)}`,
    `${shellQuote(tmuxExecutable)} -L ${shellQuote(socket)} wait-for -S ${shellQuote(doneSignal)}`,
    `${shellQuote(tmuxExecutable)} -L ${shellQuote(socket)} wait-for ${shellQuote(releaseSignal)}`,
    'exit "$status"',
    '',
  ].join('\n'), { mode: 0o700 });

  const startedAt = new Date();
  const startedNs = process.hrtime.bigint();
  let timedOut = false;
  try {
    if (markers[0] || markers[1] || clientResultMarker) {
      markerWatcher = fs.watch(artifactDirectory, () => {
        if (!fs.existsSync(timelinePath)) return;
        let outputEvents;
        try {
          outputEvents = readTimeline(timelinePath).filter((event) => event.event === 'tmux_output');
        } catch {
          return;
        }
        const visible = outputEvents.map((event) => event.text).join('');
        const recordMarker = (marker, event) => {
          const streamOffset = marker ? visible.indexOf(marker) : -1;
          if (streamOffset === -1 || observedMarkers.has(event)) return false;
          let bytes = 0;
          const outputEvent = outputEvents.find((entry) => {
            bytes += entry.text.length;
            return streamOffset < bytes;
          });
          observedMarkers.add(event);
          appendTimelineEvent(timelinePath, event, {
            source: 'client-pty', marker, stream_offset: streamOffset,
            monotonic_ns: outputEvent.monotonic_ns,
          });
          return true;
        };
        recordMarker(clientResultMarker, 'client_result');
        if (!firstMarkerReleased && markers[0] && visible.includes(markers[0])) {
          firstMarkerReleased = true;
          recordMarker(markers[0], 'marker_1');
          appendTimelineEvent(timelinePath, 'barrier_release_started', { source: 'harness' });
          barrierReleasePromise = Promise.resolve(onFirstMarker?.()).then(() => {
            appendTimelineEvent(timelinePath, 'barrier_release', { source: 'harness' });
          }).catch((error) => {
            appendTimelineEvent(timelinePath, 'barrier_release_failed', {
              source: 'harness', error: error.message,
            });
          });
        }
        if (secondMarkerTerminationStarted
          || !markers[1]
          || !visible.includes(markers[1])) return;
        recordMarker(markers[1], 'marker_2');
        if (!invocation.terminateAfterSecondMarker) return;
        secondMarkerTerminationStarted = true;
        setTimeout(() => {
          spawnResult(tmuxExecutable, ['-L', socket, 'send-keys', '-t', session, 'C-c'])
            .then(() => new Promise((resolve) => setTimeout(resolve, 200)))
            .then(() => spawnResult(tmuxExecutable, ['-L', socket, 'send-keys', '-t', session, 'C-c']))
            .catch(() => {});
        }, 500);
      });
    }
    await requireSuccess(tmuxExecutable, ['-L', socket, 'new-session', '-d', '-s', 'bootstrap']);
    for (const [name, value] of Object.entries(env)) {
      await requireSuccess(tmuxExecutable, ['-L', socket, 'set-environment', '-g', name, value]);
    }
    await requireSuccess(tmuxExecutable, [
      '-L', socket, 'new-session', '-d', '-s', session, '-c', invocation.cwd, wrapperPath,
    ]);
    const pipeCommand = [process.execPath, pipeCapturePath, timelinePath, secretsPath, pipeDonePath]
      .map(shellQuote).join(' ');
    await requireSuccess(tmuxExecutable, ['-L', socket, 'pipe-pane', '-o', '-t', session, pipeCommand]);
    appendTimelineEvent(timelinePath, 'client_started', {
      source: 'harness',
      tool_execution_owner: 'client',
      gateway_role: 'transport-only',
    });
    await requireSuccess(tmuxExecutable, ['-L', socket, 'wait-for', '-S', startSignal]);
    await requireSuccess(tmuxExecutable, ['-L', socket, 'kill-session', '-t', 'bootstrap']);
    const wait = spawnResult(tmuxExecutable, ['-L', socket, 'wait-for', doneSignal]);
    const timeout = new Promise((_, reject) => {
      setTimeout(() => {
        timedOut = true;
        reject(new Error('tmux client timing invocation timed out'));
      }, timeoutMs).unref();
    });
    await Promise.race([wait, timeout]);
    await barrierReleasePromise;
    await requireSuccess(tmuxExecutable, ['-L', socket, 'pipe-pane', '-t', session]);
    for (let attempt = 0; attempt < 20 && !fs.existsSync(pipeDonePath); attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    if (!fs.existsSync(pipeDonePath)) throw new Error('tmux pipe-pane capture did not flush');
    const pane = await requireSuccess(tmuxExecutable, ['-L', socket, 'capture-pane', '-p', '-t', session]);
    const pipeText = readTimeline(timelinePath)
      .filter((event) => event.event === 'tmux_output')
      .map((event) => event.text)
      .join('');
    const stdoutText = pipeText || (fs.existsSync(ptyPath) ? fs.readFileSync(ptyPath, 'utf8') : pane.stdout.text);
    const timingText = fs.existsSync(timingPath) ? fs.readFileSync(timingPath, 'utf8') : '';
    const exitCode = fs.existsSync(statusPath)
      ? Number.parseInt(fs.readFileSync(statusPath, 'utf8').trim(), 10)
      : 1;
    appendTimelineEvent(timelinePath, 'terminal', {
      source: 'harness', exit_code: Number.isInteger(exitCode) ? exitCode : 1,
    });
    const timeline = writeMergedTimeline(timelinePath, producerTimelinePath, secrets);
    return {
      started_at: startedAt.toISOString(),
      finished_at: new Date().toISOString(),
      duration_ms: Number(process.hrtime.bigint() - startedNs) / 1e6,
      exit_code: Number.isInteger(exitCode) ? exitCode : 1,
      signal: null,
      timed_out: false,
      stdout: { text: stdoutText, bytes: Buffer.byteLength(stdoutText), overflow: false },
      stderr: { text: pane.stderr.text, bytes: pane.stderr.bytes, overflow: pane.stderr.overflow },
      pty: {
        timing: timingText,
        pane: pane.stdout.text,
        markers: ptyMarkerTimeline(stdoutText, timingText, markers),
        observation: 'tmux-pipe-pane',
        timeline_path: timelinePath,
        timeline_events: timeline.length,
      },
    };
  } catch (error) {
    if (!timedOut) throw error;
    return {
      started_at: startedAt.toISOString(),
      finished_at: new Date().toISOString(),
      duration_ms: Number(process.hrtime.bigint() - startedNs) / 1e6,
      exit_code: null,
      signal: 'SIGKILL',
      timed_out: true,
      stdout: { text: '', bytes: 0, overflow: false },
      stderr: { text: error.message, bytes: Buffer.byteLength(error.message), overflow: false },
      pty: { timing: '', pane: '', observation: 'tmux-pipe-pane', timeline_path: timelinePath },
    };
  } finally {
    markerWatcher?.close();
    await spawnResult(tmuxExecutable, ['-L', socket, 'wait-for', '-S', releaseSignal]).catch(() => {});
    await spawnResult(tmuxExecutable, ['-L', socket, 'kill-server']).catch(() => {});
    fs.rmSync(root, { recursive: true, force: true });
  }
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
  if (client === 'opencode') {
    if (!result.stdout.text.includes(SENTINEL_RESPONSE)) {
      throw new Error(`${client} sentinel response marker was not observed`);
    }
    return 1;
  }
  const events = parseJsonLines(result.stdout.text, client);
  const compatible = client === 'codex'
    ? events.some((event) => event.type === 'item.completed'
      && event.item?.type === 'agent_message'
      && includesSentinel(event.item))
    : client === 'claude'
      ? events.some((event) => (event.type === 'assistant' || event.type === 'result')
        && includesSentinel(event))
      : events.some(includesSentinel);
  if (!compatible) throw new Error(`${client} sentinel response marker was not observed`);
  return events.length;
}

module.exports = {
  SENTINEL_RESPONSE,
  assertCompatibleResult,
  executeInvocation,
  executeTmuxInvocation,
  parseJsonLines,
  ptyMarkerTimeline,
  shellQuote,
};
