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

function output(events, exitCode = 0) {
  return {
    exit_code: exitCode,
    signal: null,
    timed_out: false,
    stdout: events.map(JSON.stringify).join('\n'),
    stderr: '',
  };
}

function claudeTextOutput(text, sessionId = 'claude-session-fixture') {
  const split = Math.floor(text.length / 2);
  return output([
    {
      type: 'stream_event', session_id: sessionId,
      event: { type: 'message_start', message: { id: 'msg-fixture' } },
    },
    {
      type: 'stream_event',
      event: { type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } },
    },
    {
      type: 'stream_event',
      event: { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: text.slice(0, split) } },
    },
    {
      type: 'stream_event',
      event: { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: text.slice(split) } },
    },
    { type: 'stream_event', event: { type: 'message_stop' } },
    { type: 'assistant', message: { content: [{ type: 'text', text }] } },
    { type: 'result', session_id: sessionId, is_error: false, terminal_reason: 'completed', result: text },
  ]);
}

function codexTextOutput(text, threadId = 'thread-fixture') {
  return output([
    { type: 'thread.started', thread_id: threadId },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'message-fixture', type: 'agent_message', text } },
    { type: 'turn.completed' },
  ]);
}

function opencodeHeadlessTextOutput(text, prompt = 'fixture user prompt') {
  const split = Math.floor(text.length / 2);
  return output([
    { type: 'message.updated', properties: { info: { id: 'user-message', role: 'user' } } },
    {
      type: 'message.part.updated',
      properties: { part: { id: 'user-part', messageID: 'user-message', type: 'text', text: prompt } },
    },
    { type: 'message.updated', properties: { info: { id: 'assistant-message', role: 'assistant' } } },
    {
      type: 'message.part.updated',
      properties: { part: { id: 'assistant-part', messageID: 'assistant-message', type: 'text', text: '' } },
    },
    {
      type: 'message.part.delta',
      properties: {
        messageID: 'assistant-message', partID: 'assistant-part', field: 'text', delta: text.slice(0, split),
      },
    },
    {
      type: 'message.part.delta',
      properties: {
        messageID: 'assistant-message', partID: 'assistant-part', field: 'text', delta: text.slice(split),
      },
    },
    {
      type: 'message.part.updated',
      properties: { part: { id: 'assistant-part', messageID: 'assistant-message', type: 'text', text } },
    },
    { type: 'session.status', properties: { status: { type: 'idle' } } },
  ]);
}

function opencodeRunTextOutput(text, sessionId = 'opencode-session-fixture') {
  return output([
    { type: 'step_start', sessionID: sessionId, part: { type: 'step-start' } },
    { type: 'text', sessionID: sessionId, part: { type: 'text', text } },
    { type: 'step_finish', sessionID: sessionId, part: { type: 'step-finish', reason: 'stop' } },
  ]);
}

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
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, codexTextOutput(TEXT_SENTINEL)).pass, true);
  assert.equal(evaluateAttempt('claude', TOOL_VECTOR, output([
    {
      type: 'assistant',
      message: { content: [{ type: 'tool_use', id: 'tool-a' }] },
    },
    {
      type: 'user',
      message: { content: [{ type: 'tool_result', tool_use_id: 'tool-a', content: TOOL_RESULT_SENTINEL }] },
    },
    {
      type: 'assistant',
      message: { content: [{ type: 'text', text: `client wrapper ${TOOL_FINAL_SENTINEL}` }] },
    },
    {
      type: 'result', is_error: false, terminal_reason: 'completed', result: TOOL_FINAL_SENTINEL,
    },
  ])).pass, true);
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
  const result = codexTextOutput(TEXT_SENTINEL);
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    ...result, stderr: 'model_client.stream_responses_websocket transport="responses_websocket"',
  }, 'responses_websocket').pass, true);
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, {
    ...result, stderr: 'model_client.stream_responses_websocket falling back to HTTP',
  }, 'responses_websocket').reason, 'responses_websocket_http_fallback');
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, result, 'responses_websocket').pass, true);
});

test('WP-D4B evaluates exact ordered text, complete continuity, and visible Provider errors', () => {
  assert.equal(evaluateAttempt('claude', LONG_TEXT_VECTOR, claudeTextOutput(LONG_REPEATED_UNICODE_TEXT)).pass, true);
  assert.equal(evaluateAttempt(
    'opencode', LONG_TEXT_VECTOR, opencodeHeadlessTextOutput(LONG_REPEATED_UNICODE_TEXT),
  ).pass, true);
  assert.equal(evaluateAttempt('codex', LONG_TEXT_VECTOR, codexTextOutput(LONG_REPEATED_UNICODE_TEXT)).pass, true);
  assert.equal(evaluateAttempt('codex', CONTINUITY_VECTOR, {
    exit_code: 0,
    signal: null,
    timed_out: false,
    stdout: '',
    stderr: '',
    turns: [CONTINUITY_SEED_SENTINEL, CONTINUITY_FINAL_SENTINEL].map((text, turnIndex) => ({
      turn_index: turnIndex,
      result: codexTextOutput(text),
    })),
  }).pass, true);
  for (const [client, surface] of [
    ['claude', claudeTextOutput],
    ['opencode', opencodeRunTextOutput],
  ]) {
    assert.equal(evaluateAttempt(client, CONTINUITY_VECTOR, {
      exit_code: 0,
      signal: null,
      timed_out: false,
      stdout: '',
      stderr: '',
      turns: [CONTINUITY_SEED_SENTINEL, CONTINUITY_FINAL_SENTINEL].map((text, turnIndex) => ({
        turn_index: turnIndex,
        result: surface(text),
      })),
    }).pass, true);
  }
  const claudeError = output([
    {
      type: 'assistant',
      message: { content: [{ type: 'text', text: `API Error: 500 ${PROVIDER_ERROR_BODY}client suffix` }] },
    },
    {
      type: 'result', is_error: true, terminal_reason: 'api_error',
      result: `API Error: 500 ${PROVIDER_ERROR_BODY}client suffix`,
    },
  ], 1);
  assert.equal(evaluateAttempt('claude', PROVIDER_ERROR_VECTOR, claudeError).pass, true);
  const encodedOpenCodeError = output([{
    type: 'error', error: { data: { message: JSON.stringify(PROVIDER_ERROR_BODY) } },
  }], 1);
  assert.equal(evaluateAttempt('opencode', PROVIDER_ERROR_VECTOR, encodedOpenCodeError).pass, true);
  assert.equal(evaluateAttempt('codex', PROVIDER_ERROR_VECTOR, output([
    { type: 'error', message: `stream disconnected before completion:${PROVIDER_ERROR_BODY}` },
    { type: 'turn.failed', error: { message: PROVIDER_ERROR_BODY } },
  ], 1)).pass, true);
  assert.equal(evaluateAttempt('claude', PROVIDER_ERROR_VECTOR, output([
    {
      type: 'assistant',
      message: { content: [{ type: 'text', text: 'future_error shape unknown keep complete body' }] },
    },
    { type: 'result', is_error: true, terminal_reason: 'api_error' },
  ], 1)).reason, 'provider_error_body_missing');
  assert.equal(evaluateAttempt('opencode', PROVIDER_ERROR_VECTOR, output([
    { type: 'message.updated', properties: { info: { id: 'user-message', role: 'user' } } },
    {
      type: 'message.part.updated',
      properties: {
        part: { id: 'user-part', messageID: 'user-message', type: 'text', text: PROVIDER_ERROR_BODY },
      },
    },
    { type: 'error', error: { data: { message: 'generic failure' } } },
  ], 1)).reason, 'provider_error_body_missing');
  assert.equal(evaluateAttempt('codex', PROVIDER_ERROR_VECTOR, output([
    { type: 'error', message: PROVIDER_ERROR_BODY },
  ], 1)).reason, 'client_terminal_missing');
});

test('F4-CLIENT-GATE controlled negatives reject prompt echo, partial text, and missing terminals', () => {
  assert.equal(evaluateAttempt(
    'opencode', TEXT_VECTOR, opencodeHeadlessTextOutput('different assistant text', TEXT_SENTINEL),
  ).reason, 'ordered_assistant_text_missing');
  assert.equal(evaluateAttempt(
    'codex', LONG_TEXT_VECTOR, codexTextOutput(`${LONG_REPEATED_UNICODE_TEXT.slice(0, -1)}x`),
  ).reason, 'ordered_assistant_text_missing');
  const noTerminal = codexTextOutput(TEXT_SENTINEL);
  noTerminal.stdout = noTerminal.stdout.split('\n').slice(0, -1).join('\n');
  assert.equal(evaluateAttempt('codex', TEXT_VECTOR, noTerminal).reason, 'ordered_assistant_text_missing');
  assert.equal(evaluateAttempt('codex', CONTINUITY_VECTOR, {
    ...codexTextOutput(`${CONTINUITY_SEED_SENTINEL}${CONTINUITY_FINAL_SENTINEL}`),
  }).reason, 'complete_conversation_missing');
  assert.equal(evaluateAttempt('codex', CONTINUITY_VECTOR, {
    exit_code: 0,
    signal: null,
    timed_out: false,
    stdout: '',
    stderr: '',
    turns: [
      { turn_index: 0, result: codexTextOutput(CONTINUITY_SEED_SENTINEL, 'thread-a') },
      { turn_index: 1, result: codexTextOutput(CONTINUITY_FINAL_SENTINEL, 'thread-b') },
    ],
  }).reason, 'complete_conversation_missing');
});

test('WP-D4B keeps Claude/OpenCode chronology strict while Codex reports completion evidence', () => {
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
    { type: 'result', is_error: false, terminal_reason: 'completed', result: PARALLEL_FINAL_SENTINEL },
  ];
  assert.equal(evaluateAttempt('claude', PARALLEL_TOOL_VECTOR, output(parallel)).pass, true);
  const claudeInterleaved = [
    { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'tool-a' }] } },
    { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 'tool-a', content: PARALLEL_RESULT_A }] } },
    { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'tool-b' }] } },
    { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 'tool-b', content: PARALLEL_RESULT_B }] } },
    parallel[2], parallel[3],
  ];
  assert.equal(
    evaluateAttempt('claude', PARALLEL_TOOL_VECTOR, output(claudeInterleaved)).reason,
    'tool_callback_evidence_missing',
  );

  const codexParallel = [
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'tool-a', type: 'command_execution' } },
    {
      type: 'item.completed',
      item: { id: 'tool-a', type: 'command_execution', aggregated_output: PARALLEL_RESULT_A },
    },
    { type: 'item.started', item: { id: 'tool-b', type: 'command_execution' } },
    {
      type: 'item.completed',
      item: { id: 'tool-b', type: 'command_execution', aggregated_output: PARALLEL_RESULT_B },
    },
    { type: 'item.completed', item: { id: 'final', type: 'agent_message', text: PARALLEL_FINAL_SENTINEL } },
    { type: 'turn.completed' },
  ];
  const codexEvidence = evaluateAttempt('codex', PARALLEL_TOOL_VECTOR, output(codexParallel));
  assert.equal(codexEvidence.pass, true);
  assert.ok(codexEvidence.observed_events.includes('codex_tool_completion_evidence_observed'));
  assert.equal(evaluateAttempt(
    'codex', PARALLEL_TOOL_VECTOR, output(codexParallel.toSpliced(4, 1)),
  ).reason, 'tool_callback_evidence_missing');
  assert.equal(evaluateAttempt(
    'codex', PARALLEL_TOOL_VECTOR, output(codexParallel.toSpliced(5, 1)),
  ).reason, 'tool_callback_evidence_missing');
  assert.equal(evaluateAttempt(
    'codex', PARALLEL_TOOL_VECTOR, output(codexParallel.toSpliced(6, 1)),
  ).reason, 'client_terminal_missing');

  const opencodeInterleaved = [
    {
      type: 'message.part.updated',
      properties: { part: { id: 'tool-a', type: 'tool', callID: 'tool-a', state: { status: 'pending' } } },
    },
    {
      type: 'message.part.updated',
      properties: {
        part: {
          id: 'tool-a', type: 'tool', callID: 'tool-a',
          state: { status: 'completed', output: PARALLEL_RESULT_A },
        },
      },
    },
    {
      type: 'message.part.updated',
      properties: { part: { id: 'tool-b', type: 'tool', callID: 'tool-b', state: { status: 'pending' } } },
    },
    {
      type: 'message.part.updated',
      properties: {
        part: {
          id: 'tool-b', type: 'tool', callID: 'tool-b',
          state: { status: 'completed', output: PARALLEL_RESULT_B },
        },
      },
    },
    { type: 'message.updated', properties: { info: { id: 'final', role: 'assistant' } } },
    {
      type: 'message.part.updated',
      properties: { part: { id: 'final-text', messageID: 'final', type: 'text', text: PARALLEL_FINAL_SENTINEL } },
    },
    { type: 'session.status', properties: { status: { type: 'idle' } } },
  ];
  assert.equal(
    evaluateAttempt('opencode', PARALLEL_TOOL_VECTOR, output(opencodeInterleaved)).reason,
    'tool_callback_evidence_missing',
  );

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
    { type: 'result', is_error: false, terminal_reason: 'completed', result: SEQUENTIAL_FINAL_SENTINEL },
  ];
  assert.equal(evaluateAttempt('claude', SEQUENTIAL_TOOL_VECTOR, output(sequential)).pass, true);
  assert.equal(evaluateAttempt('claude', SEQUENTIAL_TOOL_VECTOR, output([
    sequential[0], sequential[1], sequential[3], sequential[2], sequential[4], sequential[5],
  ])).reason, 'tool_callback_evidence_missing');
  assert.equal(evaluateAttempt('claude', PARALLEL_TOOL_VECTOR, output([
    parallel[0],
    {
      type: 'user',
      message: { content: [{ type: 'tool_result', tool_use_id: 'tool-a', content: PARALLEL_RESULT_A }] },
    },
    parallel[2], parallel[3],
  ])).reason, 'tool_callback_evidence_missing');
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
      mockSnapshot: async () => ({
        entries: [],
        counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
      }),
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
        const expectedRuns = expectation.provider_requests ?? expectation.minimum_provider_requests;
        providerRequests.push(expectedRuns);
        return { arrivals: expectedRuns, settled: expectedRuns };
      },
      waitForBarrierWaiting: async ({ before }) => {
        assert.deepEqual(before, {
          entries: [], counters: { gatewayExecutorInvocations: 0, networkObserverOutbound: 0 },
        });
        return { sequence: 1, event: 'barrier_waiting' };
      },
      verifyIdle: async () => ({ runtime_targets: 2, stream_targets: 1 }),
      vectorsFor: () => [TEXT_VECTOR, TOOL_VECTOR],
      executePlan: async (plan, execution) => {
        assert.equal(execution.onFirstMarker, undefined);
        if (plan.vector_id === TEXT_VECTOR.id) {
          if (plan.client === 'codex') return codexTextOutput(TEXT_SENTINEL);
          if (plan.client === 'claude') return claudeTextOutput(TEXT_SENTINEL);
          return opencodeHeadlessTextOutput(TEXT_SENTINEL);
        }
        if (plan.client === 'codex') return output([
          { type: 'turn.started' },
          { type: 'item.started', item: { id: 'tool-a', type: 'command_execution' } },
          {
            type: 'item.completed',
            item: { id: 'tool-a', type: 'command_execution', aggregated_output: TOOL_RESULT_SENTINEL },
          },
          { type: 'item.completed', item: { id: 'final', type: 'agent_message', text: TOOL_FINAL_SENTINEL } },
          { type: 'turn.completed' },
        ]);
        if (plan.client === 'claude') return output([
          { type: 'assistant', message: { content: [{ type: 'tool_use', id: 'tool-a' }] } },
          {
            type: 'user',
            message: { content: [{ type: 'tool_result', tool_use_id: 'tool-a', content: TOOL_RESULT_SENTINEL }] },
          },
          { type: 'assistant', message: { content: [{ type: 'text', text: TOOL_FINAL_SENTINEL }] } },
          { type: 'result', is_error: false, terminal_reason: 'completed', result: TOOL_FINAL_SENTINEL },
        ]);
        return output([
          { type: 'message.updated', properties: { info: { id: 'tool-message', role: 'assistant' } } },
          {
            type: 'message.part.updated',
            properties: {
              part: {
                id: 'tool-part', messageID: 'tool-message', type: 'tool', callID: 'tool-a',
                state: { status: 'pending' },
              },
            },
          },
          {
            type: 'message.part.updated',
            properties: {
              part: {
                id: 'tool-part', messageID: 'tool-message', type: 'tool', callID: 'tool-a',
                state: { status: 'completed', output: TOOL_RESULT_SENTINEL },
              },
            },
          },
          { type: 'message.updated', properties: { info: { id: 'final-message', role: 'assistant' } } },
          {
            type: 'message.part.updated',
            properties: {
              part: { id: 'final-part', messageID: 'final-message', type: 'text', text: TOOL_FINAL_SENTINEL },
            },
          },
          { type: 'session.status', properties: { status: { type: 'idle' } } },
        ]);
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
      network_observer_outbound: 0,
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
