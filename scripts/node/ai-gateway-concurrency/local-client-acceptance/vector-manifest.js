'use strict';

const {
  CALLBACK_RETRY_FINAL_SENTINEL,
  CALLBACK_RETRY_VECTOR_MARKER,
  CLAUDE_PROTOCOL_SENTINEL,
  CONTINUITY_FINAL_SENTINEL,
  CONTINUITY_SEED_SENTINEL,
  HTTP_500_ERROR_BODY,
  LONG_REPEATED_UNICODE_TEXT,
  PARALLEL_FINAL_SENTINEL,
  SEQUENTIAL_FINAL_SENTINEL,
  TEXT_SENTINEL,
  TOOL_FOLLOWUP_FINAL_SENTINEL,
  TOOL_FINAL_SENTINEL,
} = require('../mock-upstream/client-vector-contract');

const VECTOR_MANIFEST_SCHEMA = '1flowbase.local-client-vector-manifest/v1';
const ALL_CLIENTS = Object.freeze(['claude', 'opencode', 'codex']);

const TOOL_RESULT_SENTINEL = '1flowbase-client-tool-result';
const PARALLEL_RESULT_A = `${TOOL_RESULT_SENTINEL} parallel-a`;
const PARALLEL_RESULT_B = `${TOOL_RESULT_SENTINEL} parallel-b`;
const SEQUENTIAL_RESULT_A = `${TOOL_RESULT_SENTINEL} sequential-a`;
const SEQUENTIAL_RESULT_B = `${TOOL_RESULT_SENTINEL} sequential-b`;
const GIT_LOG_RESULT = `${TOOL_RESULT_SENTINEL} git-log`;
const GIT_SHOW_RESULT = `${TOOL_RESULT_SENTINEL} git-show`;
const GIT_WORKFLOW_FINAL = '1flowbase meaningful git workflow verified';
const PROVIDER_ERROR_BODY = HTTP_500_ERROR_BODY;
const CLAUDE_REAL_CLIENT_PROFILE_MARKER =
  '1flowbase-client-vector=claude-real-client-1m-adaptive-context-management';
const CLAUDE_PROTOCOL_PROFILE = Object.freeze({
  id: 'claude_1m_adaptive_context_management',
  model: 'claude-opus-4-6[1m]',
  effort: 'high',
  environment: Object.freeze({ USE_API_CONTEXT_MANAGEMENT: '1' }),
  expected_evidence: Object.freeze({
    configured_model: 'claude-opus-4-6[1m]',
    base_model: 'claude-opus-4-6',
    context_management: true,
  }),
});

const TOOL_ASSETS = Object.freeze({
  TOOL_PATH: Object.freeze({ filename: 'tool-vector.txt', content: `${TOOL_RESULT_SENTINEL}\n` }),
  PARALLEL_A_PATH: Object.freeze({ filename: 'parallel-a.txt', content: `${PARALLEL_RESULT_A}\n` }),
  PARALLEL_B_PATH: Object.freeze({ filename: 'parallel-b.txt', content: `${PARALLEL_RESULT_B}\n` }),
  SEQUENTIAL_A_PATH: Object.freeze({ filename: 'sequential-a.txt', content: `${SEQUENTIAL_RESULT_A}\n` }),
  SEQUENTIAL_B_PATH: Object.freeze({ filename: 'sequential-b.txt', content: `${SEQUENTIAL_RESULT_B}\n` }),
});

function successfulExpected({
  assistantTexts,
  durableRuns = 1,
  durableStatuses = null,
  providerRequests = 1,
  minimumProviderRequests = null,
  providerOutcomes = ['completed'],
  successTerminalCounts = [1],
  toolMode = null,
  toolResultMarkers = [],
  minimumCallbackResumes = null,
  callbackResumes = null,
  requestBodyKeys = [],
  requestBodyModel = null,
  requestBodyFacts = null,
  thinkingSignatureMatched = false,
  uniqueMessageIds = false,
  retryableToolResultRecovered = false,
}) {
  return Object.freeze({
    exit: 'success',
    assistant_texts: Object.freeze(assistantTexts),
    durable_runs: durableRuns,
    durable_statuses: Object.freeze(
      durableStatuses ?? Array.from({ length: durableRuns }, () => 'succeeded'),
    ),
    ...(minimumProviderRequests === null
      ? { provider_requests: providerRequests }
      : { minimum_provider_requests: minimumProviderRequests }),
    provider_outcomes: Object.freeze(providerOutcomes),
    success_terminal_counts: Object.freeze(successTerminalCounts),
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
    ...(toolMode ? {
      tool_mode: toolMode,
      tool_result_markers: Object.freeze(toolResultMarkers),
      tool_call_count: toolResultMarkers.length,
      ...(callbackResumes === null
        ? { minimum_callback_resumes: minimumCallbackResumes ?? 1 }
        : { callback_resumes: callbackResumes }),
    } : {}),
    ...(requestBodyKeys.length ? { request_body_keys: Object.freeze(requestBodyKeys) } : {}),
    ...(requestBodyModel === null ? {} : { request_body_model: requestBodyModel }),
    ...(requestBodyFacts === null ? {} : { request_body_facts: Object.freeze(requestBodyFacts) }),
    ...(thinkingSignatureMatched ? { thinking_signature_matched: true } : {}),
    ...(uniqueMessageIds ? { unique_message_ids: true } : {}),
    ...(retryableToolResultRecovered ? { retryable_tool_result_recovered: true } : {}),
  });
}

const TEXT_VECTOR = Object.freeze({
  id: 'text-canonical-sentinel',
  kind: 'text',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: `Reply with exactly: ${TEXT_SENTINEL}`,
  })]),
  expected: successfulExpected({ assistantTexts: [TEXT_SENTINEL] }),
});

const TOOL_VECTOR = Object.freeze({
  id: 'tool-two-turn',
  kind: 'tools',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-tool-vector',
      'TOOL_VECTOR_PATH={{TOOL_PATH}}',
      'Use the client-owned local read or shell tool requested by the provider.',
      `After its result is returned to the provider, print exactly: ${TOOL_FINAL_SENTINEL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [TOOL_FINAL_SENTINEL],
    minimumProviderRequests: 2,
    toolMode: 'single',
    toolResultMarkers: [TOOL_RESULT_SENTINEL],
  }),
});

const CALLBACK_RETRY_VECTOR = Object.freeze({
  id: 'tools-callback-retry-after-429',
  kind: 'tools',
  clients: Object.freeze(['claude']),
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-tool-vector',
      CALLBACK_RETRY_VECTOR_MARKER,
      'TOOL_VECTOR_PATH={{TOOL_PATH}}',
      'Use the client-owned local read tool exactly once.',
      'The Provider will reject the first tool-result continuation with HTTP 429.',
      `After the retry succeeds, print exactly: ${CALLBACK_RETRY_FINAL_SENTINEL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [CALLBACK_RETRY_FINAL_SENTINEL],
    durableRuns: 2,
    durableStatuses: ['failed', 'succeeded'],
    providerRequests: 3,
    providerOutcomes: ['completed', 'http-429', 'completed'],
    successTerminalCounts: [1, 0, 1],
    toolMode: 'callback_retry_after_429',
    toolResultMarkers: [TOOL_RESULT_SENTINEL],
    minimumCallbackResumes: 1,
    retryableToolResultRecovered: true,
  }),
});

const LONG_TEXT_VECTOR = Object.freeze({
  id: 'text-long-repeated-unicode',
  kind: 'text',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-vector=text-long-repeated-unicode',
      'Return the controlled Provider payload verbatim, preserving every repetition, space, newline,',
      'combining character, CJK character, emoji, and marker order.',
    ].join(' '),
  })]),
  expected: successfulExpected({ assistantTexts: [LONG_REPEATED_UNICODE_TEXT] }),
});

const CONTINUITY_VECTOR = Object.freeze({
  id: 'conversation-complete-continuity',
  kind: 'conversation',
  clients: ALL_CLIENTS,
  turns: Object.freeze([
    Object.freeze({
      prompt: [
        '1flowbase-client-vector=conversation-complete-continuity-seed',
        `Remember and reply with exactly: ${CONTINUITY_SEED_SENTINEL}`,
      ].join(' '),
    }),
    Object.freeze({
      prompt: [
        '1flowbase-client-vector=conversation-complete-continuity-check',
        `Using the complete prior user and assistant conversation, reply with exactly: ${CONTINUITY_FINAL_SENTINEL}`,
      ].join(' '),
    }),
  ]),
  expected: successfulExpected({
    assistantTexts: [CONTINUITY_SEED_SENTINEL, CONTINUITY_FINAL_SENTINEL],
    durableRuns: 2,
    providerRequests: 2,
  }),
});

const PROVIDER_ERROR_VECTOR = Object.freeze({
  id: 'provider-visible-error-body',
  kind: 'error',
  clients: ALL_CLIENTS,
  protocols: Object.freeze(['anthropic_sse', 'openai_chat_sse', 'responses_sse']),
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-vector=provider-visible-error-body',
      '[1flowbase-test-scenario=http-500]',
      'Surface the complete controlled Provider error body.',
    ].join(' '),
  })]),
  expected: Object.freeze({
    exit: 'failure',
    error_body: PROVIDER_ERROR_BODY,
    durable_runs: 'provider_requests',
    durable_statuses: Object.freeze(['failed']),
    minimum_provider_requests: 1,
    provider_outcomes: Object.freeze(['http-500']),
    success_terminal_counts: Object.freeze([0]),
    gateway_executor_invocations: 0,
    network_observer_outbound: 0,
  }),
});

const PARALLEL_TOOL_VECTOR = Object.freeze({
  id: 'tools-parallel-one-callback-task',
  kind: 'tools',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-tool-vector',
      '1flowbase-client-vector=tools-parallel-one-callback-task',
      'TOOL_VECTOR_PATH={{PARALLEL_A_PATH}}',
      'PARALLEL_TOOL_A_PATH={{PARALLEL_A_PATH}}',
      'PARALLEL_TOOL_B_PATH={{PARALLEL_B_PATH}}',
      'Execute both independent client-owned reads from one Provider tool-call group.',
      `Return both results in one callback task, then print exactly: ${PARALLEL_FINAL_SENTINEL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [PARALLEL_FINAL_SENTINEL],
    minimumProviderRequests: 2,
    toolMode: 'parallel_one_callback_task',
    toolResultMarkers: [PARALLEL_RESULT_A, PARALLEL_RESULT_B],
    callbackResumes: 1,
  }),
});

const SEQUENTIAL_TOOL_VECTOR = Object.freeze({
  id: 'tools-sequential-callback-tasks-one-turn',
  kind: 'tools',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-tool-vector',
      '1flowbase-client-vector=tools-sequential-callback-tasks-one-turn',
      'TOOL_VECTOR_PATH={{SEQUENTIAL_A_PATH}}',
      'SEQUENTIAL_TOOL_A_PATH={{SEQUENTIAL_A_PATH}}',
      'SEQUENTIAL_TOOL_B_PATH={{SEQUENTIAL_B_PATH}}',
      'First complete the A read callback. Only after the Provider sees A, complete the B read callback.',
      `Keep both callback tasks in this one assistant turn, then print exactly: ${SEQUENTIAL_FINAL_SENTINEL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [SEQUENTIAL_FINAL_SENTINEL],
    minimumProviderRequests: 3,
    toolMode: 'sequential_callback_tasks_one_turn',
    toolResultMarkers: [SEQUENTIAL_RESULT_A, SEQUENTIAL_RESULT_B],
    minimumCallbackResumes: 2,
  }),
});

const TOOL_HISTORY_FOLLOWUP_VECTOR = Object.freeze({
  id: 'tools-history-followup-second-turn',
  kind: 'tool_conversation',
  clients: Object.freeze(['claude']),
  turns: Object.freeze([
    Object.freeze({
      prompt: [
        '1flowbase-client-tool-vector',
        '1flowbase-client-vector=tools-history-followup-second-turn',
        'tools-sequential-callback-tasks-one-turn',
        'TOOL_VECTOR_PATH={{SEQUENTIAL_A_PATH}}',
        'SEQUENTIAL_TOOL_A_PATH={{SEQUENTIAL_A_PATH}}',
        'SEQUENTIAL_TOOL_B_PATH={{SEQUENTIAL_B_PATH}}',
        'First complete the A read callback. Only after the Provider sees A, complete the B read callback.',
        `Keep both callback tasks in this assistant turn, then print exactly: ${SEQUENTIAL_FINAL_SENTINEL}`,
      ].join(' '),
    }),
    Object.freeze({
      prompt: [
        '1flowbase-client-vector=tools-history-followup-second-turn',
        '1flowbase-client-vector=tools-history-followup-query',
        'Continue this same conversation only after preserving both prior tool results.',
        `Reply with exactly: ${TOOL_FOLLOWUP_FINAL_SENTINEL}`,
      ].join(' '),
    }),
  ]),
  expected: successfulExpected({
    assistantTexts: [SEQUENTIAL_FINAL_SENTINEL, TOOL_FOLLOWUP_FINAL_SENTINEL],
    durableRuns: 2,
    minimumProviderRequests: 4,
    toolMode: 'sequential_callbacks_then_followup',
    toolResultMarkers: [SEQUENTIAL_RESULT_A, SEQUENTIAL_RESULT_B],
    minimumCallbackResumes: 2,
    thinkingSignatureMatched: true,
    uniqueMessageIds: true,
  }),
});

const MEANINGFUL_GIT_VECTOR = Object.freeze({
  id: 'tools-meaningful-git-workflow',
  kind: 'tools',
  clients: ALL_CLIENTS,
  turns: Object.freeze([Object.freeze({
    prompt: [
      '1flowbase-client-tool-vector',
      '1flowbase-client-vector=meaningful-git-workflow',
      'GIT_REPO_PATH={{GIT_REPO_PATH}}',
      'Use client-owned local tools in the real repository.',
      'First run git log -2 --oneline. Then run git show --stat --oneline --summary HEAD.',
      'Do not modify files, commit, push, fetch, pull, or access the network.',
      `After both callback tasks complete, print exactly: ${GIT_WORKFLOW_FINAL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [GIT_WORKFLOW_FINAL],
    minimumProviderRequests: 3,
    toolMode: 'meaningful_git_workflow',
    toolResultMarkers: [GIT_LOG_RESULT, GIT_SHOW_RESULT],
    minimumCallbackResumes: 2,
  }),
});

const CLAUDE_PROTOCOL_VECTOR = Object.freeze({
  id: 'claude-1m-adaptive-context-management',
  kind: 'text',
  clients: Object.freeze(['claude']),
  protocol_profile: CLAUDE_PROTOCOL_PROFILE,
  turns: Object.freeze([Object.freeze({
    prompt: [
      CLAUDE_REAL_CLIENT_PROFILE_MARKER,
      `Reply with exactly: ${TEXT_SENTINEL}`,
    ].join(' '),
  })]),
  expected: successfulExpected({
    assistantTexts: [TEXT_SENTINEL],
    requestBodyKeys: ['context_management'],
    requestBodyModel: 'gateway-fixture-model',
    requestBodyFacts: {
      thinkingAdaptive: true,
      outputConfigEffortHigh: true,
      contextManagementPresent: true,
    },
  }),
});

const VECTOR_MANIFEST = Object.freeze({
  schema_version: VECTOR_MANIFEST_SCHEMA,
  vectors: Object.freeze([
    TEXT_VECTOR,
    TOOL_VECTOR,
    CALLBACK_RETRY_VECTOR,
    LONG_TEXT_VECTOR,
    CONTINUITY_VECTOR,
    PROVIDER_ERROR_VECTOR,
    PARALLEL_TOOL_VECTOR,
    SEQUENTIAL_TOOL_VECTOR,
    TOOL_HISTORY_FOLLOWUP_VECTOR,
    MEANINGFUL_GIT_VECTOR,
    CLAUDE_PROTOCOL_VECTOR,
  ]),
});

function vectorsFor(client, protocol) {
  return VECTOR_MANIFEST.vectors.filter((vector) => (
    vector.clients.includes(client)
      && (!vector.protocols || vector.protocols.includes(protocol))
  ));
}

module.exports = {
  CALLBACK_RETRY_FINAL_SENTINEL,
  CALLBACK_RETRY_VECTOR,
  CLAUDE_PROTOCOL_SENTINEL,
  CLAUDE_PROTOCOL_PROFILE,
  CLAUDE_PROTOCOL_VECTOR,
  CONTINUITY_FINAL_SENTINEL,
  CONTINUITY_SEED_SENTINEL,
  CONTINUITY_VECTOR,
  LONG_REPEATED_UNICODE_TEXT,
  LONG_TEXT_VECTOR,
  MEANINGFUL_GIT_VECTOR,
  GIT_LOG_RESULT,
  GIT_SHOW_RESULT,
  GIT_WORKFLOW_FINAL,
  PARALLEL_FINAL_SENTINEL,
  PARALLEL_RESULT_A,
  PARALLEL_RESULT_B,
  PARALLEL_TOOL_VECTOR,
  PROVIDER_ERROR_BODY,
  PROVIDER_ERROR_VECTOR,
  SEQUENTIAL_FINAL_SENTINEL,
  SEQUENTIAL_RESULT_A,
  SEQUENTIAL_RESULT_B,
  SEQUENTIAL_TOOL_VECTOR,
  TEXT_SENTINEL,
  TEXT_VECTOR,
  TOOL_ASSETS,
  TOOL_FINAL_SENTINEL,
  TOOL_FOLLOWUP_FINAL_SENTINEL,
  TOOL_HISTORY_FOLLOWUP_VECTOR,
  TOOL_RESULT_SENTINEL,
  TOOL_VECTOR,
  VECTOR_MANIFEST,
  VECTOR_MANIFEST_SCHEMA,
  vectorsFor,
};
