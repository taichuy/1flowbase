'use strict';

function normalizedUpdate(params) {
  const update = params?.update;
  if (!update || typeof update !== 'object') return { kind: 'unknown', raw_type: null };
  const type = update.sessionUpdate ?? null;
  if (type === 'agent_message_chunk') {
    return {
      kind: 'text_delta',
      text: update.content?.type === 'text' ? update.content.text ?? '' : '',
      raw_type: type,
    };
  }
  if (type === 'agent_thought_chunk') {
    return {
      kind: 'reasoning_delta',
      text: update.content?.type === 'text' ? update.content.text ?? '' : '',
      raw_type: type,
    };
  }
  if (type === 'tool_call' || type === 'tool_call_update') {
    return {
      kind: type,
      tool_call_id: update.toolCallId ?? update.tool_call_id ?? null,
      title: update.title ?? null,
      status: update.status ?? null,
      raw_type: type,
    };
  }
  return { kind: 'session_update', raw_type: type };
}

module.exports = { normalizedUpdate };
