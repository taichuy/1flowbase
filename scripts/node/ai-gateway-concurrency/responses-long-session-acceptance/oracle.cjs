'use strict';
const fs = require('node:fs');
const { isExpectedContextError, validateContextProbe } = require('./context-error.cjs');
const { validateInstructionOrder } = require('./instruction-order.cjs');

const MIN_SPAN_MS = 3_600_000;
const MAX_GAP_MS = 900_000;
const tool = (e) => e.method === 'item/completed' && e.params?.item?.type === 'commandExecution'
  && e.params.item.exitCode === 0 && typeof e.params.item.command === 'string'
  && /\b(rg|sed|git|nl|head)\b/.test(e.params.item.command);

function validateSubagent(events, meta) {
  const errors = [];
  const activity = events.filter((e) => e.method === 'item/completed'
    && e.params?.threadId === meta.threadId && e.params.turnId === meta.mailboxTurnId
    && e.params.item?.type === 'subAgentActivity');
  const starts = activity.filter((e) => e.params.item.kind === 'started');
  const start = starts[0];
  const childId = start?.params.item.agentThreadId;
  if (starts.length !== 1 || !childId || childId === meta.threadId)
    errors.push('missing one actual subagent start in the parent turn');
  const completion = activity.find((e) => e.params.item.kind === 'completed'
    && e.params.item.agentThreadId === childId
    && e.params.item.agentPath === start?.params.item.agentPath && e.atMs > start?.atMs);
  if (!completion) errors.push('missing correlated completed subagent result');

  let rollout = [];
  try {
    if (meta.subagentRolloutPath) rollout = fs.readFileSync(meta.subagentRolloutPath, 'utf8')
      .split('\n').filter(Boolean).map((line) => JSON.parse(line))
      .filter((record) => record && typeof record === 'object');
  } catch { rollout = []; }
  const session = rollout.find((e) => e.type === 'session_meta')?.payload;
  const childTurns = new Set(rollout.filter((e) => e.type === 'event_msg'
    && e.payload?.type === 'task_started' && e.payload.root_turn_id === meta.mailboxTurnId
    && e.payload.turn_id && e.payload.turn_id !== meta.mailboxTurnId)
    .map((e) => e.payload.turn_id));
  const contexts = rollout.filter((e) => e.type === 'turn_context'
    && childTurns.has(e.payload?.turn_id)).map((e) => e.payload);
  if (!session || session.id !== childId || session.parent_thread_id !== meta.threadId
      || !childTurns.size || [...childTurns].some((id) => !contexts.some((c) => c.turn_id === id))
      || contexts.some((c) => c?.root_turn_id !== meta.mailboxTurnId
        || c?.model !== (meta.subagentModel || 'gpt-6-sol') || c?.effort !== 'medium'))
    errors.push('missing original subagent rollout lineage or requested model/medium context');
  let activeTurn;
  let childError = false;
  for (const record of rollout) {
    if (record.type !== 'event_msg') continue;
    const event = record.payload;
    if (event?.type === 'task_started') activeTurn = event.turn_id;
    if (event?.type === 'error' && childTurns.has(activeTurn)) childError = true;
    if (event?.type === 'task_complete' && event.turn_id === activeTurn) activeTurn = undefined;
  }
  if (!childTurns.size || [...childTurns].some((id) => !rollout.some((e) => e.type === 'event_msg'
      && e.payload?.type === 'task_complete' && e.payload.turn_id === id)) || childError)
    errors.push('missing successful terminal subagent rollout');
  const successfulTurnIds = childError ? [] : [...childTurns].filter((id) => rollout.some((e) =>
    e.type === 'event_msg' && e.payload?.type === 'task_complete' && e.payload.turn_id === id));
  return { errors, start, completion, childId, successfulTurnIds };
}

function runtimeFailures(events, subagent, meta) {
  const fatal = [];
  const recovered = [];
  for (const event of events) {
    if (event.kind === 'fixture/failure' || event.kind === 'client/exit' && event.unexpected
        || event.error || event.kind === 'client/stderr'
        && /(?:sampling_error=|Turn error:)[^\n]*close_exhausted/i.test(event.text || '')) {
      fatal.push(event);
      continue;
    }
    if (event.method !== 'error') continue;
    if (isExpectedContextError(event, meta)) continue;
    const { threadId, turnId, willRetry, error } = event.params || {};
    const completed = events.some((candidate) => candidate.method === 'turn/completed'
      && candidate.params?.threadId === threadId && candidate.params?.turn?.id === turnId
      && candidate.params.turn.status === 'completed' && candidate.atMs > event.atMs)
      || threadId === subagent.childId && subagent.successfulTurnIds.includes(turnId);
    if (willRetry !== true || !threadId || !turnId || !completed
        || /close_exhausted/i.test(error?.message || '')) fatal.push(event);
    else recovered.push(event);
  }
  return { fatal, recovered };
}

function evaluate(events, meta) {
  const errors = [];
  const unverified = [];
  const root = events.filter((e) => e.params?.threadId === meta.threadId);
  const contextTurnId = meta.contextProbeRequired === true ? meta.contextProbeTurnId : undefined;
  const completed = root.filter((e) => e.method === 'turn/completed');
  const items = root.filter((e) => e.method === 'item/completed');
  const compactItem = items.find((e) => e.params.item.type === 'contextCompaction');
  const workCompleted = completed.filter((e) => e.params.turn?.id !== compactItem?.params.turnId
    && e.params.turn?.id !== contextTurnId);
  const invoked = events.filter((e) => e.kind === 'turn/invoked' && e.threadId === meta.threadId
    && e.turnId !== contextTurnId);
  const ids = invoked.map((e) => e.turnId);
  const good = workCompleted.filter((e) => e.params.turn?.status === 'completed' && ids.includes(e.params.turn.id));
  const sourceTools = items.filter(tool);
  const firstTool = sourceTools[0];
  const lastTool = sourceTools.at(-1);
  if (!meta.threadId || new Set(ids).size !== ids.length || ids.length !== workCompleted.length
      || workCompleted.some((e) => !ids.includes(e.params.turn?.id))) errors.push('thread/turn correlation incomplete');
  if (completed.some((e) => e.params.turn?.id !== contextTurnId
      && e.params.turn?.status !== 'completed')) errors.push('non-completed turn');
  const requiredSpan = meta.minSpanMs ?? MIN_SPAN_MS;
  if (!Number.isSafeInteger(requiredSpan) || requiredSpan < 1_800_000)
    errors.push('invalid required work span (minimum 30 minutes)');
  if (!Number.isFinite(meta.durationMs) || meta.durationMs < requiredSpan)
    errors.push(`duration below ${requiredSpan / 60_000} minutes`);
  if (!firstTool || !lastTool || lastTool.atMs - firstTool.atMs < requiredSpan)
    errors.push(`useful tool work span below ${requiredSpan / 60_000} minutes`);
  if (good.length < 4) errors.push('insufficient useful turns');
  for (let i = 1; i < sourceTools.length; i++) {
    if (sourceTools[i].atMs - sourceTools[i - 1].atMs > MAX_GAP_MS) errors.push('idle useful-work gap exceeds 15 minutes');
  }
  const subagent = validateSubagent(events, meta);
  const runtime = runtimeFailures(events, subagent, meta);
  const context = validateContextProbe(events, meta);
  errors.push(...context.errors);
  errors.push(...validateInstructionOrder(events, meta));
  if (runtime.fatal.length)
    errors.push('runtime close_exhausted/error or fixture failure');
  if (events.some((e) => e.method === 'model/rerouted')) errors.push('model rerouting');
  if (items.some((e) => e.params.item.type === 'fileChange')) errors.push('client attempted file change');

  errors.push(...subagent.errors);

  const compactRequest = events.find((e) => e.kind === 'rpc/request' && e.method === 'thread/compact/start'
    && e.params?.threadId === meta.threadId);
  const compact = items.find((e) => e.params.item.type === 'contextCompaction'
    && e.atMs > (compactRequest?.atMs ?? Infinity));
  if (!compact || !completed.some((e) => e.params.turn?.id === compact.params.turnId
      && e.params.turn.status === 'completed' && e.atMs >= compact.atMs))
    errors.push('missing explicit completed compaction');
  const continuityTurn = meta.continuityTurnId;
  const continuation = good.find(e => e.params.turn.id === continuityTurn
    && e.atMs > (compact?.atMs ?? Infinity));
  const continuationRequest = invoked.find(e => e.turnId === continuityTurn
    && e.atMs > (compact?.atMs ?? Infinity));
  if (!continuation || !continuationRequest || continuation.atMs <= continuationRequest.atMs)
    errors.push('missing successful same-thread post-compaction continuity');

  const steerRequest = events.find((e) => e.kind === 'rpc/request' && e.method === 'turn/steer'
    && e.params?.threadId === meta.threadId && e.params.expectedTurnId === meta.steerTurnId);
  const steerAccepted = events.find((e) => e.kind === 'steer/accepted' && e.turnId === meta.steerTurnId
    && e.atMs > (steerRequest?.atMs ?? Infinity));
  const steerTerminal = good.find(e => e.params.turn.id === meta.steerTurnId);
  if (!steerAccepted || !steerTerminal || !(steerRequest.atMs > (compact?.atMs ?? Infinity))
      || !(steerAccepted.atMs < steerTerminal.atMs))
    errors.push('active-turn steer lifecycle order invalid');

  // A mailbox interrupt is a separate client/runtime observation, never inferred from model prose.
  const mailbox = meta.mailboxEvidence;
  let mailboxEvents = [];
  try {
    if (mailbox?.rawLogPath) mailboxEvents = fs.readFileSync(mailbox.rawLogPath, 'utf8').trim()
      .split('\n').filter(Boolean).map((line) => JSON.parse(line));
  } catch { mailboxEvents = []; }
  const childId = subagent.childId;
  const childDone = mailboxEvents.find((e) => e.kind === 'mailbox/child-completed'
    && e.rootThreadId === meta.threadId && e.parentTurnId === meta.mailboxTurnId
    && e.childThreadId === childId && Number.isFinite(e.atMs)
    && e.atMs >= (subagent.start?.atMs ?? Infinity) && e.sourceEventId);
  const before = mailboxEvents.find((e) => e.kind === 'parent/native-sampling-before'
    && e.rootThreadId === meta.threadId && e.parentTurnId === meta.mailboxTurnId
    && e.flowRunId && e.rolloutId && e.parentSampleId && e.sourceEventId
    && Number.isFinite(e.atMs) && e.atMs < (childDone?.atMs ?? -Infinity));
  const reconnect = mailboxEvents.find((e) => e.kind === 'gateway/websocket-reconnected'
    && e.rootThreadId === meta.threadId && e.parentTurnId === meta.mailboxTurnId
    && e.previousFlowRunId === before?.flowRunId && e.rolloutId === before?.rolloutId
    && e.flowRunId && e.sourceEventId && e.originalPayload?.flow_run_id === e.flowRunId
    && (e.originalPayload?.payload || e.originalPayload?.raw_json_payloads)
    && Number.isFinite(e.atMs) && e.atMs > (childDone?.atMs ?? Infinity));
  const after = mailboxEvents.find((e) => e.kind === 'parent/native-sampling-after'
    && e.rootThreadId === meta.threadId && e.parentTurnId === meta.mailboxTurnId
    && e.flowRunId === reconnect?.flowRunId && e.rolloutId === before?.rolloutId
    && e.parentSampleId && e.parentSampleId !== before?.parentSampleId && e.sourceEventId
    && Number.isFinite(e.atMs) && e.atMs > (reconnect?.atMs ?? Infinity));
  const parentTerminal = completed.find((e) => e.params.turn.id === meta.mailboxTurnId);
  if (!mailbox?.rawLogPath || !childDone || !before || !reconnect || !after
      || !(after.atMs < (parentTerminal?.atMs ?? -Infinity))
      || !root.some((e) => e.params.turnId === meta.mailboxTurnId && e.atMs >= after.atMs))
    unverified.push('mailbox-triggered parent sampling interruption and resumption');
  return { status: errors.length ? 'FAIL' : unverified.length ? 'UNVERIFIED' : 'PASS',
    pass: errors.length === 0 && unverified.length === 0, errors, unverified,
    usefulTurns: good.length, durationMs: meta.durationMs, recoveredRetryNotifications: runtime.recovered.length,
    usefulWorkSpanMs: firstTool && lastTool ? lastTool.atMs - firstTool.atMs : 0,
    requiredWorkSpanMs: requiredSpan, contextProbeVerified: context.verified };
}

module.exports = { evaluate, validateSubagent, isUsefulTool: tool, MIN_SPAN_MS };
