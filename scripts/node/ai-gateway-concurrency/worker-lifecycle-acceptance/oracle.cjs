'use strict';

function assertKillIdentity(worker, expected, parent) {
  if (!Number.isInteger(worker.pid) || worker.pid <= 1 || worker.pid === expected.parentPid
      || worker.ppid !== expected.parentPid || parent.pid !== expected.parentPid
      || parent.exe !== expected.parentExe || worker.exe !== expected.workerExe
      || worker.digest !== expected.workerDigest || worker.startTime !== expected.startTime
      || !worker.argv.includes(expected.workerArg) || !expected.workerArg
      || !parent.argv.includes(expected.parentArg) || !expected.parentArg
      || parent.argv.some((arg) => arg.includes('7800'))) {
    throw new Error('Unsafe supplier identity: PID, PPID, executable, digest, start time or exact argv mismatch');
  }
  return worker.pid;
}

function evaluate(events, meta) {
  const errors = [];
  const root = events.filter((e) => e.params?.threadId === meta.threadId);
  const completed = root.filter((e) => e.method === 'turn/completed');
  const invoked = new Set(events.filter((e) => e.kind === 'turn/invoked').map((e) => e.turnId));
  const good = completed.filter((e) => e.params.turn.status === 'completed' && invoked.has(e.params.turn.id));
  const items = root.filter((e) => e.method === 'item/completed');
  if (meta.durationMs < 5400000) errors.push('duration below 5400 seconds');
  if (!items.some((e) => e.params.item.type === 'collabAgentToolCall'
    && e.params.item.tool === 'spawnAgent' && e.params.item.status === 'completed'
    && e.params.item.receiverThreadIds?.length)) errors.push('missing actual subagent spawn');
  if (!items.some((e) => e.params.item.type === 'collabAgentToolCall'
    && e.params.item.tool === 'wait' && e.params.item.status === 'completed'
    && Object.values(e.params.item.agentsStates || {}).some((s) => s.status === 'completed')))
    errors.push('missing completed subagent result');
  if (!events.some((e) => e.kind === 'rpc/request' && e.method === 'thread/compact/start')
      || !items.some((e) => e.params.item.type === 'contextCompaction')) errors.push('missing explicit completed compaction');
  const steer = events.find((e) => e.kind === 'steer/accepted');
  if (!steer || !items.some((e) => e.params.turnId === steer.turnId
    && e.params.item.type === 'agentMessage' && e.params.item.text?.includes(meta.nonce)))
    errors.push('missing processed active-turn steer nonce');
  if (!meta.sentinelBefore || !meta.sentinelAfter || meta.sentinelBefore !== meta.sentinelAfter)
    errors.push('sentinel/tool-result persistence missing');
  for (const e of good) {
    const own = items.filter((item) => item.params.turnId === e.params.turn.id);
    if (!own.some((item) => item.params.item.type === 'commandExecution' && item.params.item.exitCode === 0)
      || !own.some((item) => item.params.item.type === 'agentMessage' && item.params.item.text?.length >= 100))
      errors.push(`turn ${e.params.turn.id} lacks successful tool-backed useful analysis`);
  }
  if (good.length < 4) errors.push('insufficient useful turns');
  if (good.length && good.at(-1).atMs - good[0].atMs < 5400000) errors.push('useful work span below 5400 seconds');
  for (let i = 1; i < good.length; i++) if (good[i].atMs - good[i - 1].atMs > 900000)
    errors.push('useful work gap exceeds 15 minutes');
  if (!meta.betweenKill?.before || !meta.betweenKill?.after || meta.betweenKill.before.pid === meta.betweenKill.after.pid)
    errors.push('missing between-turn supplier replacement');
  const kill = meta.midKill;
  if (!kill?.before || !kill.semanticSeen || !kill.after || kill.before.pid === kill.after.pid) errors.push('missing midcall semantic-output kill/replacement');
  if (kill) {
    const terminal = completed.filter((e) => e.params.turn.id === kill.turnId);
    if (terminal.length !== 1 || terminal[0].params.turn.status !== 'failed') errors.push('midcall must terminally fail once');
    if (!good.some((e) => e.atMs > kill.atMs && e.params.turn.id !== kill.turnId)) errors.push('missing lawful new successful turn');
    if (root.some((e) => e.method === 'error' && e.params.turnId === kill.turnId && e.params.willRetry))
      errors.push('hidden client retry detected');
  }
  if (events.some((e) => e.method === 'model/rerouted')) errors.push('unexpected model rerouting');
  if (items.some((e) => e.params.item.type === 'fileChange')) errors.push('model attempted file writes');
  return { pass: errors.length === 0, errors, usefulTurns: good.length, durationMs: meta.durationMs };
}
module.exports = { assertKillIdentity, evaluate };
