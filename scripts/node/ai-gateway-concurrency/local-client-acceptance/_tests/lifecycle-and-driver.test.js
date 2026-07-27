'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const {
  CONTINUITY_VECTOR, LONG_TEXT_VECTOR, PARALLEL_TOOL_VECTOR, PROVIDER_ERROR_VECTOR,
  SEQUENTIAL_TOOL_VECTOR, TEXT_SENTINEL, TEXT_VECTOR, TOOL_FINAL_SENTINEL,
  TOOL_RESULT_SENTINEL, TOOL_VECTOR,
} = require('../contract');
const {
  CONTINUITY_FINAL_SENTINEL, CONTINUITY_SEED_SENTINEL, LONG_REPEATED_UNICODE_TEXT,
  PARALLEL_FINAL_SENTINEL, PARALLEL_RESULT_A, PARALLEL_RESULT_B, PROVIDER_ERROR_BODY,
  SEQUENTIAL_FINAL_SENTINEL, SEQUENTIAL_RESULT_A, SEQUENTIAL_RESULT_B,
} = require('../vector-manifest');
const { evaluateAttempt, publicPlan, runLocalClientAcceptance } = require('../driver');
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

test('WP-D4B public command artifacts redact key and embedded authorization configuration', () => {
  const command = publicPlan({
    client_surface: 'fixture-surface',
    invocation: { executable: '/machine/client', args: [], cwd: '/tmp/client' },
    environment: {
      ANTHROPIC_API_KEY: 'fixture-key',
      OPENCODE_CONFIG_CONTENT: '{"apiKey":"fixture-key"}',
      USE_API_CONTEXT_MANAGEMENT: '1',
    },
    configFiles: [],
  }, 'tmux');
  assert.equal(command.environment.ANTHROPIC_API_KEY, '<redacted>');
  assert.equal(command.environment.OPENCODE_CONFIG_CONTENT, '<isolated-config>');
  assert.equal(command.environment.USE_API_CONTEXT_MANAGEMENT, '1');
  assert.doesNotMatch(JSON.stringify(command), /fixture-key/u);
});

test('WP-14A canonical text and tool two-turn evaluations require observable evidence', () => {
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: JSON.stringify({ type: 'item.completed', item: { type: 'agent_message', text: TEXT_SENTINEL } }),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('claude', TOOL_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: [
      { type: 'assistant', message: { content: [{ type: 'tool_use' }] } },
      { type: 'user', message: { content: [{ type: 'tool_result', content: TOOL_RESULT_SENTINEL }] } },
      { type: 'assistant', message: { content: [{ type: 'text', text: TOOL_FINAL_SENTINEL }] } },
    ].map(JSON.stringify).join('\n'),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('opencode', TOOL_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: JSON.stringify({
      type: 'message.part.updated',
      properties: { part: { type: 'text', text: TOOL_FINAL_SENTINEL } },
    }),
    stderr: '',
  }).pass, false);
});

test('WP-14A Codex WebSocket evidence rejects fallback without requiring internal INFO logs', () => {
  const output = JSON.stringify({
    type: 'item.completed', item: { type: 'agent_message', text: TEXT_SENTINEL },
  });
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    exit_code: 0, timed_out: false, stdout: output,
    stderr: 'model_client.stream_responses_websocket transport="responses_websocket"',
  }, 'responses_websocket').pass, true);
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    exit_code: 0, timed_out: false, stdout: output,
    stderr: 'model_client.stream_responses_websocket falling back to HTTP',
  }, 'responses_websocket').reason, 'responses_websocket_http_fallback');
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    exit_code: 0, timed_out: false, stdout: output, stderr: '',
  }, 'responses_websocket').pass, true);
});

test('WP-D4B evaluates exact ordered text, complete continuity, and visible Provider errors', () => {
  assert.equal(evaluateAttempt('codex', LONG_TEXT_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: JSON.stringify({
      type: 'item.completed', item: { type: 'agent_message', text: LONG_REPEATED_UNICODE_TEXT },
    }),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('codex', CONTINUITY_VECTOR, {
    exit_code: 0,
    timed_out: false,
    stdout: [CONTINUITY_SEED_SENTINEL, CONTINUITY_FINAL_SENTINEL].map((text) => JSON.stringify({
      type: 'item.completed', item: { type: 'agent_message', text },
    })).join('\n'),
    stderr: '',
  }).pass, true);
  assert.equal(evaluateAttempt('claude', PROVIDER_ERROR_VECTOR, {
    exit_code: 1,
    timed_out: false,
    stdout: '',
    stderr: PROVIDER_ERROR_BODY,
  }).pass, true);
  assert.equal(evaluateAttempt('claude', PROVIDER_ERROR_VECTOR, {
    exit_code: 1,
    timed_out: false,
    stdout: '',
    stderr: 'keep complete body',
  }).reason, 'provider_error_body_missing');
});

test('WP-D4B distinguishes parallel results from sequential callback tasks in one turn', () => {
  const parallel = [
    { type: 'assistant', message: { content: [
      { type: 'tool_use', id: 'tool-a' },
      { type: 'tool_use', id: 'tool-b' },
    ] } },
    { type: 'user', message: { content: [
      { type: 'tool_result', tool_use_id: 'tool-a', content: PARALLEL_RESULT_A },
      { type: 'tool_result', tool_use_id: 'tool-b', content: PARALLEL_RESULT_B },
    ] } },
    { type: 'assistant', message: { content: [{ type: 'text', text: PARALLEL_FINAL_SENTINEL }] } },
  ];
  assert.equal(evaluateAttempt('claude', PARALLEL_TOOL_VECTOR, {
    exit_code: 0, timed_out: false, stdout: parallel.map(JSON.stringify).join('\n'), stderr: '',
  }).pass, true);

  const sequential = [
    { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'tool-a' }] } },
    { type: 'user', message: { content: [
      { type: 'tool_result', tool_use_id: 'tool-a', content: SEQUENTIAL_RESULT_A },
    ] } },
    { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'tool-b' }] } },
    { type: 'user', message: { content: [
      { type: 'tool_result', tool_use_id: 'tool-a', content: SEQUENTIAL_RESULT_A },
      { type: 'tool_result', tool_use_id: 'tool-b', content: SEQUENTIAL_RESULT_B },
    ] } },
    { type: 'assistant', message: { content: [{ type: 'text', text: SEQUENTIAL_FINAL_SENTINEL }] } },
  ];
  assert.equal(evaluateAttempt('claude', SEQUENTIAL_TOOL_VECTOR, {
    exit_code: 0, timed_out: false, stdout: sequential.map(JSON.stringify).join('\n'), stderr: '',
  }).pass, true);
});

test('WP-14A driver emits mock-backed reconciliation evidence and cleans resources in finally', async () => {
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
  const reconciled = [];
  const providerRequests = [];
  let toolBarrierReleases = 0;
  try {
    const result = await runLocalClientAcceptance({
      artifactRoot: path.join(root, 'artifacts'),
      tmuxExecutable: bin,
      discovery: { env: { PATH: root } },
      targets: Object.fromEntries(['claude', 'opencode', 'codex'].map((client) => [client, {
        applicationId: `${client}-app`,
        model: 'fixture-model',
        apiKey: `sk-${client}-secret-value`,
        gatewayBaseUrl: 'http://127.0.0.1:4567',
        durable: { list_runs: {}, query_run: {} },
        runtimeActivity: {},
        activeStreams: {},
      }])),
      mockSnapshot: async () => ({ entries: [], counters: { gatewayExecutorInvocations: 0 } }),
      releaseBarrier: async () => { toolBarrierReleases += 1; },
    }, {
      registry,
      discoverClients: () => discovered,
      probeVersion: async (binary) => ({ status: 'ready', version: `${path.basename(binary)} 1.0`, reason: null }),
      snapshotRuns: async () => ({ ids: [], runs: [] }),
      reconcileAttempt: async ({ expectedRuns }) => {
        reconciled.push(expectedRuns);
        return {
          runs: Array.from({ length: expectedRuns }, (_, index) => ({
            id: `run-${index}`,
            status: 'succeeded',
          })),
        };
      },
      evaluateMockAttempt: (_before, _after, expectation) => {
        const expectedRuns = expectation.provider_requests;
        providerRequests.push(expectedRuns);
        return { arrivals: expectedRuns, settled: expectedRuns };
      },
      waitForBarrierWaiting: async ({ before }) => {
        assert.deepEqual(before, { entries: [], counters: { gatewayExecutorInvocations: 0 } });
        return { sequence: 1, event: 'barrier_waiting' };
      },
      verifyIdle: async () => ({ runtime_targets: 2, stream_targets: 1 }),
      vectorsFor: () => [TEXT_VECTOR, TOOL_VECTOR],
      executePlan: async (plan, execution) => {
        assert.equal(execution.onFirstMarker, undefined);
        const clientEvents = {
          codex: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'item.completed', item: { type: 'agent_message', text: TEXT_SENTINEL } }]
            : [
              { type: 'item.completed', item: { type: 'command_execution' } },
              { type: 'item.completed', item: { type: 'command_execution', output: TOOL_RESULT_SENTINEL } },
              { type: 'item.completed', item: { type: 'agent_message', text: TOOL_FINAL_SENTINEL } },
            ],
          claude: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'assistant', message: { content: [{ type: 'text', text: TEXT_SENTINEL }] } }]
            : [
              { type: 'assistant', message: { content: [{ type: 'tool_use' }] } },
              { type: 'user', message: { content: [{ type: 'tool_result', content: TOOL_RESULT_SENTINEL }] } },
              { type: 'assistant', message: { content: [{ type: 'text', text: TOOL_FINAL_SENTINEL }] } },
            ],
          opencode: plan.vector_id === TEXT_VECTOR.id
            ? [{ type: 'message.part.updated', properties: { part: { type: 'text', text: TEXT_SENTINEL } } }]
            : [
              { type: 'message.part.updated', properties: { part: { type: 'tool' } } },
              { type: 'message.part.updated', properties: { part: { type: 'tool', output: TOOL_RESULT_SENTINEL } } },
              { type: 'message.part.updated', properties: { part: { type: 'text', text: TOOL_FINAL_SENTINEL } } },
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
    assert.equal(result.gate_role, 'mock_backed_local_client_acceptance');
    assert.equal(result.vector_manifest.schema_version, '1flowbase.local-client-vector-manifest/v1');
    assert.ok(result.vector_manifest.vector_ids.includes(SEQUENTIAL_TOOL_VECTOR.id));
    assert.equal(cleaned, true);
    const artifact = fs.readFileSync(result.artifact_path, 'utf8');
    assert.doesNotMatch(artifact, /sk-(claude|opencode|codex)-secret-value/u);
    assert.deepEqual(result.clients.find((client) => client.name === 'codex').protocols, [
      'responses_sse', 'responses_websocket',
    ]);
    assert.deepEqual(reconciled, [1, 1, 1, 1, 1, 1, 1, 1]);
    assert.deepEqual(providerRequests, [1, 2, 1, 2, 1, 2, 1, 2]);
    assert.equal(toolBarrierReleases, 4);
    assert.deepEqual(result.final_reconciliation, {
      runtime_targets: 2,
      stream_targets: 1,
      gateway_executor_invocations: 0,
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
