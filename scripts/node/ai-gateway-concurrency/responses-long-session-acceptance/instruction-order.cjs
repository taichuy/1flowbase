'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { databaseRows } = require('./mailbox-evidence.cjs');

function orderedUserInput(body, nonces) {
  const users = (body?.native_request?.wire_body?.input || []).filter(item => item.role === 'user');
  const text = users.map(item => typeof item.content === 'string' ? item.content
    : (item.content || []).map(part => part.text || '').join('\n')).join('\n');
  const positions = nonces.map(nonce => text.indexOf(nonce));
  return positions.every((position, index) => position >= 0 && (!index || position > positions[index - 1]));
}

function validateInstructionOrder(events, meta) {
  if (!meta.orderedSteerRequired) return [];
  const errors = [];
  const requests = events.filter(e => e.kind === 'rpc/request' && e.method === 'turn/steer'
    && e.params?.threadId === meta.threadId && e.params.expectedTurnId === meta.steerTurnId);
  const accepted = events.filter(e => e.kind === 'steer/accepted' && e.turnId === meta.steerTurnId);
  const terminal = events.find(e => e.method === 'turn/completed' && e.params?.threadId === meta.threadId
    && e.params.turn?.id === meta.steerTurnId && e.params.turn.status === 'completed');
  if (requests.length !== 2 || accepted.length !== 2 || !terminal
      || !meta.steerNonces?.every((nonce, index) =>
        requests[index]?.params.input?.some(item => item.text?.includes(nonce))
        && accepted[index]?.ordinal === index + 1
        && accepted[index].atMs > requests[index].atMs
        && accepted[index].atMs < (requests[index + 1]?.atMs ?? terminal.atMs)))
    errors.push('two active-turn instructions were not accepted in submission order');
  const evidence = meta.instructionOrderEvidence;
  if (evidence?.status !== 'COLLECTED' || evidence.rootThreadId !== meta.threadId
      || evidence.turnId !== meta.steerTurnId || !evidence.sourceEventId || !evidence.flowRunId
      || evidence.nonceOrder?.join('|') !== meta.steerNonces?.join('|')
      || !(evidence.atMs > (accepted[1]?.atMs ?? Infinity) && evidence.atMs < (terminal?.atMs ?? -Infinity)))
    errors.push('missing original Native request evidence for both ordered instructions');
  return errors;
}

function collectInstructionOrder({ events, meta, out, repositoryRoot }) {
  if (!meta.orderedSteerRequired) return null;
  let evidence = { status: 'UNVERIFIED' };
  try {
    const childId = events.find(e => e.params?.threadId === meta.threadId
      && e.params.item?.type === 'subAgentActivity' && e.params.item.kind === 'started')?.params.item.agentThreadId;
    const rows = databaseRows({ ...meta, mailboxTurnId: meta.steerTurnId }, repositoryRoot, childId);
    for (const event of rows.original) {
      if (event.originalPayload?.kind !== 'model_call') continue;
      const raw = event.originalPayload.body;
      const body = typeof raw === 'string' ? JSON.parse(raw) : raw;
      if (!orderedUserInput(body, meta.steerNonces)) continue;
      evidence = { status: 'COLLECTED', rootThreadId: meta.threadId, turnId: meta.steerTurnId,
        sourceEventId: event.id, flowRunId: event.flowRunId,
        atMs: Date.parse(event.createdAt) - Date.parse(meta.startedUtc), nonceOrder: meta.steerNonces,
        bodyDigest: crypto.createHash('sha256').update(JSON.stringify(body)).digest('hex') };
      break;
    }
  } catch { /* Missing original evidence remains explicitly unverified. */ }
  fs.writeFileSync(path.join(out, 'instruction-order-evidence.json'), JSON.stringify(evidence), { mode: 0o600 });
  return evidence;
}

module.exports = { orderedUserInput, validateInstructionOrder, collectInstructionOrder };
