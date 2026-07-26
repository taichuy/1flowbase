'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { LONG_REPEAT_COUNT, LONG_SEGMENT, TEXT_VECTOR, TOOL_VECTOR } = require('../contract');
const { evaluateAttempt, runLocalClientAcceptance } = require('../driver');
const { OwnedResources, executionEnvironment } = require('../lifecycle');

test('AC-009 cleanup terminates owned children, tmux servers, and temporary roots', async () => {
  const killed = [];
  const tmux = [];
  const removed = [];
  const child = {
    exitCode: null,
    signalCode: null,
    kill(signal) { killed.push(signal); },
    once() {},
  };
  const resources = new OwnedResources({
    spawnSync(executable, args) { tmux.push([executable, ...args]); return { status: 0 }; },
    rmSync(root) { removed.push(root); },
  });
  resources.addChild(child);
  resources.addTmuxSocket('owned-socket');
  resources.addTempRoot('/tmp/owned-root');
  assert.deepEqual(await resources.close(), []);
  assert.deepEqual(killed, ['SIGTERM', 'SIGKILL']);
  assert.deepEqual(tmux, [['tmux', '-L', 'owned-socket', 'kill-server']]);
  assert.deepEqual(removed, ['/tmp/owned-root']);
});

test('AC-009 child environment carries gateway config without inheriting host credentials', () => {
  const environment = executionEnvironment({
    invocation: { cwd: '/tmp/isolated/output' },
    environment: { ANTHROPIC_API_KEY: 'ephemeral-key' },
  }, {
    PATH: '/machine/bin',
    OPENAI_API_KEY: 'host-secret',
    CLAUDE_CODE_OAUTH_TOKEN: 'host-oauth',
  });
  assert.equal(environment.PATH, '/machine/bin');
  assert.equal(environment.HOME, '/tmp/isolated');
  assert.equal(environment.ANTHROPIC_API_KEY, 'ephemeral-key');
  assert.equal(environment.OPENAI_API_KEY, undefined);
  assert.equal(environment.CLAUDE_CODE_OAUTH_TOKEN, undefined);
});

test('AC-009 long/repeated and tool two-turn evaluations require observable evidence', () => {
  const text = Array(LONG_REPEAT_COUNT).fill(LONG_SEGMENT).join(' ');
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: JSON.stringify({ type: 'item.completed', item: { type: 'agent_message', text } }),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('claude', TOOL_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: [
      { type: 'assistant', message: { content: [{ type: 'tool_use' }] } },
      { type: 'user', message: { content: [{ type: 'tool_result', content: '1flowbase-tool-result-challenge' }] } },
      { type: 'assistant', message: { content: [{ type: 'text', text: '1flowbase-tool-two-turn-complete' }] } },
    ].map(JSON.stringify).join('\n'),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('opencode', TOOL_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: JSON.stringify({
      type: 'message.part.updated',
      properties: { part: { type: 'text', text: '1flowbase-tool-two-turn-complete' } },
    }),
    stderr: '',
  }).pass, false);
});

test('AC-009 driver emits a non-blocking artifact and cleans resources in finally', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'local-client-driver-test-'));
  const bin = path.join(root, 'tmux');
  fs.writeFileSync(bin, '#!/bin/sh\n', { mode: 0o700 });
  let cleaned = false;
  const registry = {
    roots: [],
    addTempRoot(value) { this.roots.push(value); return value; },
    async close() {
      for (const value of this.roots) fs.rmSync(value, { recursive: true, force: true });
      cleaned = true;
      return [];
    },
  };
  const discovered = Object.fromEntries(['claude', 'opencode', 'codex'].map((client) => [client, {
    client,
    status: 'ready',
    reason: null,
    binary: `/machine/${client}`,
    config_path: `/machine/config/${client}`,
  }]));
  try {
    const result = await runLocalClientAcceptance({
      artifactRoot: path.join(root, 'artifacts'),
      tmuxExecutable: bin,
      discovery: { env: { PATH: root } },
      targets: Object.fromEntries(['claude', 'opencode', 'codex'].map((client) => [client, {
        model: 'fixture-model',
        apiKey: `sk-${client}-secret-value`,
        gatewayBaseUrl: 'http://127.0.0.1:4567',
      }])),
    }, {
      registry,
      discoverClients: () => discovered,
      probeVersion: async (binary) => ({ status: 'ready', version: `${path.basename(binary)} 1.0`, reason: null }),
      executePlan: async (plan) => {
        const text = Array(LONG_REPEAT_COUNT).fill(LONG_SEGMENT).join(' ');
        const clientEvents = {
          codex: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'item.completed', item: { type: 'agent_message', text } }]
            : [
              { type: 'item.completed', item: { type: 'command_execution' } },
              { type: 'item.completed', item: { type: 'command_execution', output: '1flowbase-tool-result-challenge' } },
              { type: 'item.completed', item: { type: 'agent_message', text: '1flowbase-tool-two-turn-complete' } },
            ],
          claude: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'assistant', message: { content: [{ type: 'text', text }] } }]
            : [
              { type: 'assistant', message: { content: [{ type: 'tool_use' }] } },
              { type: 'user', message: { content: [{ type: 'tool_result', content: '1flowbase-tool-result-challenge' }] } },
              { type: 'assistant', message: { content: [{ type: 'text', text: '1flowbase-tool-two-turn-complete' }] } },
            ],
          opencode: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'message.part.updated', properties: { part: { type: 'text', text } } }]
            : [
              { type: 'message.part.updated', properties: { part: { type: 'tool' } } },
              { type: 'message.part.updated', properties: { part: { type: 'tool', output: '1flowbase-tool-result-challenge' } } },
              { type: 'message.part.updated', properties: { part: { type: 'text', text: '1flowbase-tool-two-turn-complete' } } },
            ],
        };
        return {
          exit_code: 0,
          signal: null,
          timed_out: false,
          stdout: clientEvents[plan.client].map(JSON.stringify).join('\n'),
          stderr: '',
        };
      },
    });
    assert.equal(result.status, 'pass');
    assert.equal(result.gate_role, 'explicit_non_blocking_local_client_acceptance');
    assert.equal(cleaned, true);
    const artifact = fs.readFileSync(result.artifact_path, 'utf8');
    assert.doesNotMatch(artifact, /sk-(claude|opencode|codex)-secret-value/u);
    assert.deepEqual(result.clients.find((client) => client.name === 'codex').protocols, [
      'responses_sse', 'responses_websocket',
    ]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
