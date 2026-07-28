'use strict';

const CODEX_TOOL_ITEM_TYPES = new Set(['command_execution', 'mcp_tool_call', 'tool_call']);

function structuredEvents(output) {
  return String(output).split(/\r?\n/u).flatMap((line) => {
    const trimmed = line.replace(/\u001b\[[0-?]*[ -/]*[@-~]/gu, '').trim();
    if (!trimmed.startsWith('{')) return [];
    try { return [JSON.parse(trimmed)]; } catch { return []; }
  });
}

function nestedStrings(value, output = []) {
  if (typeof value === 'string') output.push(value);
  else if (Array.isArray(value)) {
    for (const item of value) nestedStrings(item, output);
  } else if (value && typeof value === 'object') {
    for (const item of Object.values(value)) nestedStrings(item, output);
  }
  return output;
}

function decodedStringCandidates(value) {
  const candidates = [value];
  let current = value;
  for (let depth = 0; depth < 2; depth += 1) {
    try {
      const decoded = JSON.parse(current);
      if (typeof decoded === 'string' && decoded !== current) {
        candidates.push(decoded);
        current = decoded;
        continue;
      }
      candidates.push(...nestedStrings(decoded));
    } catch {
      break;
    }
    break;
  }
  return candidates;
}

function decodedVisibleErrorStrings(client, result) {
  const stdout = result?.stdout || '';
  const stderr = result?.stderr || '';
  const events = structuredEvents(stdout);
  const eventStrings = events.flatMap((event) => {
    if (client === 'claude') {
      if (event.type === 'assistant') return nestedStrings(event.message?.content ?? []);
      return event.type === 'result' && typeof event.result === 'string' ? [event.result] : [];
    }
    if (client === 'codex') {
      if (event.type === 'error') return nestedStrings(event);
      return event.type === 'turn.failed' ? nestedStrings(event.error ?? {}) : [];
    }
    return event.type === 'error' ? nestedStrings(event.error ?? event) : [];
  });
  return [stderr, ...(events.length === 0 ? [stdout] : []), ...eventStrings]
    .flatMap((value) => decodedStringCandidates(value));
}

function claudeAssistantTexts(events) {
  const streamed = [];
  let message = null;
  for (const [index, outer] of events.entries()) {
    if (outer.type !== 'stream_event' || !outer.event) continue;
    const event = outer.event;
    if (event.type === 'message_start') message = { parts: new Map() };
    if (!message) continue;
    if (event.type === 'content_block_start' && event.content_block?.type === 'text') {
      message.parts.set(event.index, event.content_block.text || '');
    } else if (event.type === 'content_block_delta' && event.delta?.type === 'text_delta') {
      message.parts.set(event.index, `${message.parts.get(event.index) || ''}${event.delta.text || ''}`);
    } else if (event.type === 'message_stop') {
      const text = [...message.parts.entries()].sort(([left], [right]) => left - right)
        .map(([, value]) => value).join('');
      if (text) streamed.push({ index, text });
      message = null;
    }
  }
  if (streamed.length) return streamed;
  const completed = events.flatMap((event, index) => {
    if (event.type !== 'assistant') return [];
    const text = (event.message?.content ?? [])
      .filter((part) => part.type === 'text' && typeof part.text === 'string')
      .map((part) => part.text).join('');
    return text ? [{ index, text }] : [];
  });
  if (completed.length) return completed;
  return events.flatMap((event, index) => (
    event.type === 'result' && typeof event.result === 'string'
      ? [{ index, text: event.result }]
      : []
  ));
}

function codexAssistantTexts(events) {
  return events.flatMap((event, index) => {
    if (event.type !== 'item.completed' || event.item?.type !== 'agent_message') return [];
    if (typeof event.item.text === 'string') return [{ index, text: event.item.text }];
    if (typeof event.item.content === 'string') return [{ index, text: event.item.content }];
    if (!Array.isArray(event.item.content)) return [];
    const text = event.item.content
      .filter((part) => part?.type === 'output_text' && typeof part.text === 'string')
      .map((part) => part.text).join('');
    return text ? [{ index, text }] : [];
  });
}

function opencodeAssistantTexts(events) {
  const messageRoles = new Map();
  for (const event of events) {
    const info = event.type === 'message.updated' ? event.properties?.info : null;
    if (typeof info?.id === 'string' && typeof info.role === 'string') {
      messageRoles.set(info.id, info.role);
    }
  }
  const parts = new Map();
  const direct = [];
  for (const [index, event] of events.entries()) {
    if (event.type === 'text' && event.part?.type === 'text' && typeof event.part.text === 'string') {
      direct.push({ index, text: event.part.text });
      continue;
    }
    if (event.type === 'message.part.delta' && event.properties?.field === 'text') {
      const messageId = event.properties.messageID;
      const partId = event.properties.partID;
      if (messageRoles.get(messageId) !== 'assistant' || typeof partId !== 'string') continue;
      const current = parts.get(partId) || { index, text: '' };
      current.text += event.properties.delta || '';
      current.index = index;
      parts.set(partId, current);
      continue;
    }
    const part = event.type === 'message.part.updated' ? event.properties?.part : null;
    if (part?.type !== 'text' || messageRoles.get(part.messageID) !== 'assistant') continue;
    if (typeof part.id !== 'string' || typeof part.text !== 'string') continue;
    const current = parts.get(part.id) || { index, text: '' };
    current.text = part.text;
    current.index = index;
    parts.set(part.id, current);
  }
  return [...direct, ...parts.values()].filter(({ text }) => text.length > 0)
    .sort((left, right) => left.index - right.index);
}

function clientAssistantTexts(client, events) {
  if (client === 'claude') return claudeAssistantTexts(events);
  if (client === 'codex') return codexAssistantTexts(events);
  if (client === 'opencode') return opencodeAssistantTexts(events);
  throw new Error(`unsupported local client surface: ${client}`);
}

function clientTerminal(client, events, expectedExit) {
  if (client === 'claude') {
    const index = events.findLastIndex((event) => event.type === 'result');
    const event = events[index];
    const success = event?.is_error === false && event?.terminal_reason === 'completed';
    const failure = event?.is_error === true && event?.terminal_reason === 'api_error';
    return { index, observed: expectedExit === 'success' ? success : failure };
  }
  if (client === 'codex') {
    const terminalType = expectedExit === 'success' ? 'turn.completed' : 'turn.failed';
    const index = events.findLastIndex((event) => event.type === terminalType);
    return { index, observed: index >= 0 };
  }
  const terminalType = expectedExit === 'success' ? 'step_finish' : 'error';
  let index = events.findLastIndex((event) => event.type === terminalType);
  if (expectedExit === 'success' && index === -1) {
    index = events.findLastIndex((event) => (
      event.type === 'session.status' && event.properties?.status?.type === 'idle'
    ));
  }
  return { index, observed: index >= 0 };
}

function clientSurface(client, result, expectedExit) {
  const events = structuredEvents(result?.stdout || '');
  return {
    events,
    assistantTexts: clientAssistantTexts(client, events),
    terminal: clientTerminal(client, events, expectedExit),
  };
}

function conversationId(client, result) {
  const events = structuredEvents(result?.stdout || '');
  if (client === 'claude') {
    return events.find((event) => typeof event.session_id === 'string')?.session_id ?? null;
  }
  if (client === 'codex') {
    return events.find((event) => event.type === 'thread.started' && typeof event.thread_id === 'string')
      ?.thread_id ?? null;
  }
  return events.find((event) => typeof event.sessionID === 'string')?.sessionID
    ?? events.find((event) => typeof event.properties?.sessionID === 'string')?.properties.sessionID
    ?? events.find((event) => typeof event.part?.sessionID === 'string')?.part.sessionID
    ?? null;
}

function claudeToolEntries(event, index) {
  const calls = event.type === 'assistant'
    ? (event.message?.content ?? []).filter((part) => part.type === 'tool_use')
      .flatMap((part) => typeof part.id === 'string' ? [{ id: part.id, index, value: part }] : [])
    : [];
  const results = event.type === 'user'
    ? (event.message?.content ?? []).filter((part) => part.type === 'tool_result')
      .flatMap((part) => typeof part.tool_use_id === 'string'
        ? [{ id: part.tool_use_id, index, value: part }]
        : [])
    : [];
  return { calls, results };
}

function codexToolEntries(event, index) {
  if (!CODEX_TOOL_ITEM_TYPES.has(event.item?.type) || typeof event.item.id !== 'string') {
    return { calls: [], results: [] };
  }
  const calls = ['item.started', 'item.completed'].includes(event.type)
    ? [{ id: event.item.id, index, value: event.item }]
    : [];
  const results = event.type === 'item.completed'
    ? [{ id: event.item.id, index, value: event.item }]
    : [];
  return { calls, results };
}

function opencodeToolEntries(event, index) {
  const part = event.type === 'message.part.updated'
    ? event.properties?.part
    : event.type === 'tool_use' ? event.part : null;
  if (!part || !['tool', 'tool_call'].includes(part.type)) return { calls: [], results: [] };
  const id = part.callID || part.callId || part.id;
  if (typeof id !== 'string') return { calls: [], results: [] };
  return {
    calls: [{ id, index, value: part }],
    results: part.state?.status === 'completed' ? [{ id, index, value: part }] : [],
  };
}

function toolEntries(client, events) {
  const extract = client === 'claude'
    ? claudeToolEntries
    : client === 'codex' ? codexToolEntries : opencodeToolEntries;
  const calls = new Map();
  const results = [];
  for (const [index, event] of events.entries()) {
    const entries = extract(event, index);
    for (const call of entries.calls) if (!calls.has(call.id)) calls.set(call.id, call);
    results.push(...entries.results);
  }
  return { calls: [...calls.values()], results };
}

function resultForMarker(results, marker) {
  return results.find((result) => nestedStrings(result.value).some((value) => value.includes(marker))) ?? null;
}

function evaluateToolSurface(client, vector, surface) {
  const { calls, results } = toolEntries(client, surface.events);
  const markers = vector.expected.tool_result_markers;
  const expectedCallCount = vector.expected.tool_call_count;
  const markedResults = markers.map((marker) => resultForMarker(results, marker));
  const uniqueResultIds = new Set(markedResults.filter(Boolean).map((result) => result.id));
  const callsById = new Map(calls.map((call) => [call.id, call]));
  const paired = markers.length === expectedCallCount
    && markedResults.every((result) => result && callsById.has(result.id))
    && uniqueResultIds.size === markers.length
    && calls.length === expectedCallCount;
  // Codex reports local scheduler order; mock evidence owns the Provider callback-group proof.
  const codexParallelCompletionEvidence = paired
    && client === 'codex'
    && vector.expected.tool_mode === 'parallel_one_callback_task';
  let chronology = false;
  if (!codexParallelCompletionEvidence
    && paired && [
      'sequential_callback_tasks_one_turn',
      'meaningful_git_workflow',
    ].includes(vector.expected.tool_mode)) {
    const [firstResult, secondResult] = markedResults;
    chronology = callsById.get(firstResult.id).index < firstResult.index
      && firstResult.index < callsById.get(secondResult.id).index
      && callsById.get(secondResult.id).index < secondResult.index;
  } else if (!codexParallelCompletionEvidence
    && paired && vector.expected.tool_mode === 'parallel_one_callback_task') {
    chronology = calls.every((call) => call.index < Math.min(...markedResults.map((result) => result.index)));
  } else if (!codexParallelCompletionEvidence && paired) {
    chronology = callsById.get(markedResults[0].id).index < markedResults[0].index;
  }
  const lastResultIndex = markedResults.every(Boolean)
    ? Math.max(...markedResults.map((result) => result.index))
    : -1;
  const finalMarker = vector.expected.assistant_texts.at(-1);
  const final = surface.assistantTexts.find(({ index, text }) => (
    index > lastResultIndex && text.includes(finalMarker)
  ));
  const terminalAfterFinal = Boolean(final)
    && surface.terminal.observed
    && surface.terminal.index > final.index;
  const pass = paired && (chronology || codexParallelCompletionEvidence) && terminalAfterFinal;
  const observed = [];
  if (calls.length === expectedCallCount) observed.push('tool_calls_observed');
  if (markedResults.every(Boolean) && uniqueResultIds.size === markers.length) {
    observed.push('tool_results_observed');
  }
  if (codexParallelCompletionEvidence) observed.push('codex_tool_completion_evidence_observed');
  else if (chronology) observed.push(`${vector.expected.tool_mode}_observed`);
  if (final) observed.push('final_marker_observed');
  if (terminalAfterFinal) observed.push('client_terminal_observed');
  return { pass, reason: pass ? null : 'tool_callback_evidence_missing', observed_events: observed };
}

function expectedTurnResults(vector, result) {
  const turns = Array.isArray(result.turns) ? result.turns.map((turn) => turn.result) : [result];
  return turns.length === vector.turns.length ? turns : null;
}

function evaluateFailureSurface(client, vector, result) {
  const turns = expectedTurnResults(vector, result);
  if (!turns || result.exit_code === 0) {
    return { pass: false, reason: 'provider_error_body_missing', observed_events: [] };
  }
  const terminal = turns.every((turn) => clientSurface(client, turn, 'failure').terminal.observed);
  const visible = turns.some((turn) => (
    decodedVisibleErrorStrings(client, turn).some((value) => value.includes(vector.expected.error_body))
  ));
  const pass = terminal && visible;
  return {
    pass,
    reason: pass ? null : terminal ? 'provider_error_body_missing' : 'client_terminal_missing',
    observed_events: [
      ...(visible ? ['provider_error_body_observed'] : []),
      ...(terminal ? ['client_terminal_observed'] : []),
    ],
  };
}

function evaluateTextSurface(client, vector, result) {
  const turns = expectedTurnResults(vector, result);
  if (!turns) return { pass: false, reason: 'complete_conversation_missing', observed_events: [] };
  if (turns.length > 1) {
    const ids = turns.map((turn) => conversationId(client, turn));
    if (ids.some((id) => id === null) || new Set(ids).size !== 1) {
      return { pass: false, reason: 'complete_conversation_missing', observed_events: [] };
    }
  }
  const exact = turns.every((turn, index) => {
    const surface = clientSurface(client, turn, 'success');
    return surface.terminal.observed
      && surface.assistantTexts.length === 1
      && surface.assistantTexts[0].text === vector.expected.assistant_texts[index]
      && surface.terminal.index > surface.assistantTexts[0].index;
  });
  return {
    pass: exact,
    reason: exact ? null : 'ordered_assistant_text_missing',
    observed_events: exact
      ? ['ordered_assistant_text_observed', 'client_terminal_observed']
      : [],
  };
}

function evaluateAttempt(client, vector, result, protocol = null) {
  if (result.timed_out) return { pass: false, reason: 'client_process_timed_out', observed_events: [] };
  const combined = `${result.stdout || ''}\n${result.stderr || ''}`;
  if (vector.expected.exit === 'failure') return evaluateFailureSurface(client, vector, result);
  if (result.exit_code !== 0) {
    return { pass: false, reason: 'client_process_failed', observed_events: [] };
  }
  if (client === 'codex' && protocol === 'responses_websocket'
    && /falling back to HTTP|fallback_to_http/iu.test(combined)) {
    return { pass: false, reason: 'responses_websocket_http_fallback', observed_events: [] };
  }
  const turns = expectedTurnResults(vector, result);
  if (!turns) return { pass: false, reason: 'complete_conversation_missing', observed_events: [] };
  if (vector.kind === 'tools') {
    const surface = clientSurface(client, turns[0], 'success');
    if (!surface.terminal.observed) {
      return { pass: false, reason: 'client_terminal_missing', observed_events: [] };
    }
    return evaluateToolSurface(client, vector, surface);
  }
  return evaluateTextSurface(client, vector, result);
}

module.exports = {
  clientAssistantTexts,
  clientSurface,
  decodedVisibleErrorStrings,
  evaluateAttempt,
  structuredEvents,
  toolEntries,
};
