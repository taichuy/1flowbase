'use strict';

const HTTP_500_ERROR_BODY =
  ' \n{"future_error":{"shape":"unknown"},"message":"keep complete body"}\n ';
const TEXT_SENTINEL = '1flowbase gateway sentinel ok';
const TOOL_FINAL_SENTINEL = '1flowbase gateway tool sentinel ok';
const PARALLEL_FINAL_SENTINEL = '1flowbase parallel callback sentinel ok';
const SEQUENTIAL_FINAL_SENTINEL = '1flowbase sequential callback sentinel ok';
const TOOL_FOLLOWUP_FINAL_SENTINEL = '1flowbase tool history followup sentinel ok';
const GIT_WORKFLOW_FINAL = '1flowbase meaningful git workflow verified';
const CONTINUITY_SEED_SENTINEL = '1flowbase continuity seed 中🙂';
const CONTINUITY_FINAL_SENTINEL = '1flowbase complete conversation continuity ok';
const CLAUDE_PROTOCOL_SENTINEL = '1flowbase claude protocol context ok';
const LONG_TEXT_BEGIN = '1flowbase-long-unicode-begin\n';
const LONG_TEXT_UNIT = '重复段🙂🚀|e\u0301|漢字|`same`  **same**|  same  same  \n';
const LONG_TEXT_END = '1flowbase-long-unicode-end';
const LONG_REPEATED_UNICODE_TEXT = `${LONG_TEXT_BEGIN}${LONG_TEXT_UNIT.repeat(1024)}${LONG_TEXT_END}`;

function containsValue(value, marker) {
  if (typeof value === 'string') return value.includes(marker);
  if (Array.isArray(value)) return value.some((item) => containsValue(item, marker));
  if (value && typeof value === 'object') return Object.values(value).some((item) => containsValue(item, marker));
  return false;
}

function hasClaudeProtocolProfile(body) {
  return body?.model === 'claude-opus-4-6'
    && body?.thinking?.type === 'adaptive'
    && body?.output_config?.effort === 'high'
    && body?.context_management !== undefined;
}

function textVectorOutput(body, knownContinuityResponses = new Set()) {
  if (containsValue(body, '1flowbase-client-vector=text-long-repeated-unicode')) {
    return LONG_REPEATED_UNICODE_TEXT;
  }
  if (containsValue(body, '1flowbase-client-vector=conversation-complete-continuity-check')) {
    const hasCompleteHistory = containsValue(body, CONTINUITY_SEED_SENTINEL)
      || knownContinuityResponses.has(body?.previous_response_id);
    return hasCompleteHistory ? CONTINUITY_FINAL_SENTINEL : null;
  }
  if (containsValue(body, '1flowbase-client-vector=conversation-complete-continuity-seed')) {
    return CONTINUITY_SEED_SENTINEL;
  }
  if (containsValue(body, '1flowbase-client-vector=claude-1m-adaptive-context-management')) {
    return hasClaudeProtocolProfile(body) ? CLAUDE_PROTOCOL_SENTINEL : null;
  }
  if (containsValue(body, TEXT_SENTINEL)) return TEXT_SENTINEL;
  return null;
}

function toolVectorFinalOutput(body) {
  if (containsValue(body, 'tools-history-followup-query')) return TOOL_FOLLOWUP_FINAL_SENTINEL;
  if (containsValue(body, '1flowbase-client-vector=meaningful-git-workflow')) return GIT_WORKFLOW_FINAL;
  if (containsValue(body, 'tools-parallel-one-callback-task')) return PARALLEL_FINAL_SENTINEL;
  if (containsValue(body, 'tools-sequential-callback-tasks-one-turn')) return SEQUENTIAL_FINAL_SENTINEL;
  return TOOL_FINAL_SENTINEL;
}

function hasClosedHistoricalToolPairs(body, minimumPairs = 2) {
  const calls = new Set();
  const results = new Set();
  const visit = (value) => {
    if (Array.isArray(value)) {
      for (const item of value) visit(item);
      return;
    }
    if (!value || typeof value !== 'object') return;
    if (value.type === 'function_call') {
      const id = value.call_id ?? value.id;
      if (typeof id === 'string') calls.add(id);
    }
    if (value.type === 'function_call_output' && typeof value.call_id === 'string') {
      results.add(value.call_id);
    }
    if (value.type === 'tool_use' && typeof value.id === 'string') calls.add(value.id);
    if (value.type === 'tool_result' && typeof value.tool_use_id === 'string') {
      results.add(value.tool_use_id);
    }
    for (const toolCall of Array.isArray(value.tool_calls) ? value.tool_calls : []) {
      if (typeof toolCall?.id === 'string') calls.add(toolCall.id);
    }
    if (value.role === 'tool' && typeof value.tool_call_id === 'string') {
      results.add(value.tool_call_id);
    }
    for (const nested of Object.values(value)) visit(nested);
  };
  visit(body);
  const paired = [...calls].filter((id) => results.has(id));
  const containsGatewayCallbackWrapper = paired.some((id) => (
    id.startsWith('toolu_task_') || id.startsWith('calltask_')
  ));
  return paired.length >= minimumPairs && !containsGatewayCallbackWrapper;
}

module.exports = {
  CLAUDE_PROTOCOL_SENTINEL,
  CONTINUITY_FINAL_SENTINEL,
  CONTINUITY_SEED_SENTINEL,
  HTTP_500_ERROR_BODY,
  LONG_REPEATED_UNICODE_TEXT,
  GIT_WORKFLOW_FINAL,
  PARALLEL_FINAL_SENTINEL,
  SEQUENTIAL_FINAL_SENTINEL,
  TOOL_FOLLOWUP_FINAL_SENTINEL,
  TEXT_SENTINEL,
  TOOL_FINAL_SENTINEL,
  containsValue,
  hasClaudeProtocolProfile,
  hasClosedHistoricalToolPairs,
  textVectorOutput,
  toolVectorFinalOutput,
};
