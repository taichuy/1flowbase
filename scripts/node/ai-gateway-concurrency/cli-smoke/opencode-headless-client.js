#!/usr/bin/env node
'use strict';

const net = require('node:net');
const { spawn } = require('node:child_process');

const MAX_DIAGNOSTIC_BYTES = 64 * 1024;

function parseArguments(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined) throw new Error('invalid headless OpenCode arguments');
    values[key.slice(2)] = value;
  }
  for (const key of ['opencode', 'directory', 'model', 'prompt']) {
    if (!values[key]) throw new Error(`missing --${key}`);
  }
  return values;
}

function splitModel(value) {
  const [providerID, ...model] = value.split('/');
  if (!providerID || model.length === 0) throw new Error('OpenCode model must use provider/model');
  return { providerID, modelID: model.join('/') };
}

function sessionCreateBody() {
  return {
    title: '1flowbase gateway acceptance',
    permission: [
      { permission: 'read', pattern: '*', action: 'allow' },
      { permission: 'question', pattern: '*', action: 'deny' },
      { permission: 'plan_enter', pattern: '*', action: 'deny' },
      { permission: 'plan_exit', pattern: '*', action: 'deny' },
    ],
  };
}

function promptBody(model, prompt) {
  return {
    agent: 'build',
    model: splitModel(model),
    parts: [{ type: 'text', text: prompt }],
  };
}

async function* parseSseStream(body) {
  if (!body) throw new Error('OpenCode event stream omitted a body');
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let pending = '';
  while (true) {
    const { done, value } = await reader.read();
    pending += decoder.decode(value || new Uint8Array(), { stream: !done });
    const frames = pending.split(/\r?\n\r?\n/u);
    pending = frames.pop() || '';
    for (const frame of frames) {
      const data = frame.split(/\r?\n/u)
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trimStart())
        .join('\n');
      if (data && data !== '[DONE]') yield JSON.parse(data);
    }
    if (done) break;
  }
}

function reservePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      server.close((error) => error ? reject(error) : resolve(address.port));
    });
  });
}

function boundedDiagnostics(child) {
  let value = '';
  const append = (chunk) => {
    value = (value + chunk.toString('utf8')).slice(-MAX_DIAGNOSTIC_BYTES);
  };
  child.stdout.on('data', append);
  child.stderr.on('data', append);
  return () => value;
}

function waitForListening(child, diagnostics, endpoint, timeoutMs = 20_000) {
  if (diagnostics().includes(endpoint)) return Promise.resolve();
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      child.stdout.removeListener('data', observe);
      child.stderr.removeListener('data', observe);
      child.removeListener('exit', exited);
      if (error) reject(error);
      else resolve();
    };
    const observe = () => {
      if (diagnostics().includes(endpoint)) finish();
    };
    const exited = (code) => finish(new Error(`OpenCode server exited before readiness (${code})`));
    const timer = setTimeout(() => finish(new Error('OpenCode server readiness timed out')), timeoutMs);
    child.stdout.on('data', observe);
    child.stderr.on('data', observe);
    child.once('exit', exited);
    observe();
  });
}

async function request(url, options, label) {
  const response = await fetch(url, options);
  if (!response.ok) {
    const body = (await response.text()).slice(0, 2000);
    throw new Error(`${label} returned HTTP ${response.status}: ${body}`);
  }
  if (response.status === 204) return null;
  return response.json();
}

async function stopChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill('SIGTERM');
  await Promise.race([
    new Promise((resolve) => child.once('exit', resolve)),
    new Promise((resolve) => setTimeout(resolve, 2000)),
  ]);
  if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
}

async function runHeadlessClient(options) {
  const port = await reservePort();
  const origin = `http://127.0.0.1:${port}`;
  const child = spawn(options.opencode, [
    'serve', '--hostname', '127.0.0.1', '--port', String(port),
  ], {
    cwd: options.directory,
    env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const diagnostics = boundedDiagnostics(child);
  const query = `directory=${encodeURIComponent(options.directory)}`;
  const headers = {
    'content-type': 'application/json',
    'x-opencode-directory': options.directory,
  };
  try {
    await waitForListening(child, diagnostics, origin);
    await request(`${origin}/global/health`, { headers }, 'OpenCode health');
    const eventResponse = await fetch(`${origin}/event?${query}`, { headers });
    if (!eventResponse.ok) throw new Error(`OpenCode event subscription returned HTTP ${eventResponse.status}`);
    const session = await request(`${origin}/session?${query}`, {
      method: 'POST', headers, body: JSON.stringify(sessionCreateBody()),
    }, 'OpenCode session creation');
    if (!session?.id) throw new Error('OpenCode session creation omitted an id');
    await request(`${origin}/session/${encodeURIComponent(session.id)}/prompt_async?${query}`, {
      method: 'POST', headers, body: JSON.stringify(promptBody(options.model, options.prompt)),
    }, 'OpenCode prompt');

    let active = false;
    for await (const event of parseSseStream(eventResponse.body)) {
      if (event?.properties?.sessionID !== session.id && event?.type !== 'server.connected') continue;
      process.stdout.write(`${JSON.stringify(event)}\n`);
      if (event.type === 'session.error') throw new Error('OpenCode session emitted an error');
      if (event.type === 'message.part.updated') active = true;
      if (event.type === 'session.status' && event.properties?.status?.type === 'busy') active = true;
      if (active && event.type === 'session.status' && event.properties?.status?.type === 'idle') return;
    }
    throw new Error('OpenCode event stream ended before the session became idle');
  } finally {
    await stopChild(child);
  }
}

async function main() {
  await runHeadlessClient(parseArguments(process.argv.slice(2)));
}

if (require.main === module) {
  main().catch((error) => {
    process.stderr.write(`${error.stack || error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = {
  parseArguments,
  parseSseStream,
  promptBody,
  runHeadlessClient,
  sessionCreateBody,
  splitModel,
};
