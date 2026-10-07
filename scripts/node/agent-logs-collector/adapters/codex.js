'use strict';
// Format evidence: codex-rs/history/src/{lib,rollout_payload}.rs and
// codex-rs/protocol/src/protocol.rs. No user rollout is bundled or read implicitly.
function text(value) {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.map(item => item.text ?? '').join('\n');
  return value == null ? null : JSON.stringify(value);
}
function usage(value, basis, responseId) {
  const result = { basis };
  if (responseId) result.response_id = responseId;
  for (const [from, to] of Object.entries({ input_tokens: 'input_tokens', output_tokens: 'output_tokens',
    cached_input_tokens: 'input_cache_hit_tokens', cache_write_input_tokens: 'cache_write_tokens', total_tokens: 'total_tokens' })) {
    if (Number.isSafeInteger(value?.[from]) && value[from] >= 0) result[to] = value[from];
  }
  return result;
}
module.exports = {
  sourceClient: 'codex',
  createContext(first) {
    if (first.type !== 'session_meta' || !first.payload?.id) throw new Error('Codex rollout must begin with session_meta and id');
    const meta = first.payload;
    return { session: meta.id, provider: meta.model_provider ?? null, model: null, turn: null, parent: null,
      inheritedBefore: meta.subagent_history_start_ordinal ?? meta.forked_from_ordinal_exclusive ?? null };
  },
  convert(line, context, position) {
    const p = line.payload ?? {};
    const inherited = Boolean(line.metadata?.inherited_user_message || p.inherited ||
      (context.inheritedBefore != null && line.ordinal != null && line.ordinal < context.inheritedBefore));
    const explicitTurn = inherited ? null : p.turn_id ?? p.internal_chat_message_metadata_passthrough?.turn_id;
    if (explicitTurn) context.turn = explicitTurn;
    if (!inherited && (line.type === 'turn_context' || (line.type === 'event_msg' && ['task_started', 'turn_started'].includes(p.type)))) {
      context.turn = p.turn_id ?? context.turn;
      context.parent = p.parent_turn_id ?? (p.root_turn_id !== context.turn ? p.root_turn_id : null) ?? null;
      context.model = p.model ?? context.model;
      context.provider = p.model_provider ?? context.provider;
    }
    if (!inherited && line.type === 'token_usage_record') {
      context.turn = p.turn_id ?? context.turn;
      context.parent = p.root_turn_id && p.root_turn_id !== context.turn ? p.root_turn_id : context.parent;
    }
    const event = {
      source_session_id: context.session, source_task_id: explicitTurn ?? context.turn,
      parent_source_task_id: context.parent, occurred_at: line.timestamp,
      kind: 'context', content: null, phase: null, name: null, call_id: null,
      model_id: context.model, provider_code: context.provider, usage: null,
      inherited, raw: line,
    };
    if (line.type === 'response_item') {
      if (p.type === 'message') {
        event.kind = { user: 'user', assistant: 'assistant', system: 'system', developer: 'system' }[p.role] ?? 'context';
        event.content = text(p.content); event.phase = p.phase ?? null;
      } else if (['function_call', 'custom_tool_call', 'local_shell_call', 'web_search_call', 'image_generation_call'].includes(p.type)) {
        event.kind = 'tool_call'; event.name = p.name ?? p.type; event.call_id = p.call_id ?? p.id ?? null;
        event.content = text(p.arguments ?? p.input ?? p.action);
      } else if (['function_call_output', 'custom_tool_call_output', 'local_shell_call_output'].includes(p.type)) {
        event.kind = 'tool_result'; event.call_id = p.call_id ?? null; event.content = text(p.output);
      }
    } else if (line.type === 'token_usage_record') {
      event.kind = 'usage'; event.usage = usage(p.usage, 'delta', p.response_id);
      // Turn/thread totals remain intact in raw, never added to the response delta.
    } else if (line.type === 'event_msg') {
      if (p.type === 'token_count' && p.info?.total_token_usage) {
        // Session/thread history observation with no response attribution. The
        // backend must not add it to task totals or infer a per-turn increment.
        event.kind = 'usage'; event.usage = usage(p.info.total_token_usage, 'cumulative');
      } else if (p.type === 'user_message' || p.type === 'agent_message') {
        // event_msg is a presentation mirror of response_item in standard rollouts.
        // Keep it in trajectory to avoid guessing content-based deduplication.
        event.content = text(p.message); event.phase = p.phase ?? null;
      } else if (['task_complete', 'turn_complete', 'turn_aborted'].includes(p.type)) {
        event.kind = 'task_end';
        if (p.type === 'turn_aborted') event.phase = 'cancelled';
      }
    }
    if (typeof event.occurred_at !== 'string' || !Number.isFinite(Date.parse(event.occurred_at))) throw new Error('Rollout event lacks a valid source timestamp');
    if (!inherited && event.kind === 'task_end') { context.turn = null; context.parent = null; }
    return event;
  },
};
