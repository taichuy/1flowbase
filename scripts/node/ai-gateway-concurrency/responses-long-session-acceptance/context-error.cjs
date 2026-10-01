'use strict';

const contextInfo = error => error?.codexErrorInfo === 'contextWindowExceeded';

function isExpectedContextError(event, meta) {
  return meta.contextProbeRequired === true && Boolean(meta.contextProbeTurnId) && event.method === 'error'
    && event.params?.threadId === meta.threadId
    && event.params.turnId === meta.contextProbeTurnId
    && event.params.willRetry === false && contextInfo(event.params.error);
}

// Only the explicitly requested negative turn can fail; no other final error is excused.
function validateContextProbe(events, meta) {
  if (!meta.contextProbeRequired) return { errors: [], verified: false };
  const errors = [];
  const id = meta.contextProbeTurnId;
  const selected = events.filter(e => e.params?.threadId === meta.threadId
    && (e.params.turnId === id || e.params.turn?.id === id));
  const invoked = events.filter(e => e.kind === 'turn/invoked'
    && e.threadId === meta.threadId && e.turnId === id);
  const notice = selected.filter(e => e.method === 'error');
  const terminal = selected.filter(e => e.method === 'turn/completed');
  const request = events.find(e => e.kind === 'rpc/request' && e.method === 'turn/start'
    && e.params?.threadId === meta.threadId
    && e.contextProbeInput?.sha256 === meta.contextProbeInput?.sha256
    && e.contextProbeInput?.bytes === meta.contextProbeInput?.bytes
    && events.some(response => response.kind === 'rpc/receive' && response.id === e.id
      && response.result?.turn?.id === id && response.atMs > e.atMs));
  if (!id || invoked.length !== 1 || !request
      || request.contextProbeInput.sha256 !== meta.contextProbeInput?.sha256
      || request.contextProbeInput.bytes !== meta.contextProbeInput?.bytes
      || !(request.contextProbeInput.bytes > 0)) errors.push('missing correlated context probe input');
  if (notice.length !== 1 || !isExpectedContextError(notice[0], meta))
    errors.push('context probe was not one non-retryable ContextWindowExceeded');
  if (terminal.length !== 1 || terminal[0].params.turn.status !== 'failed'
      || !contextInfo(terminal[0].params.turn.error)
      || !(terminal[0].atMs >= (notice[0]?.atMs ?? Infinity)))
    errors.push('missing failed context terminal after its error notification');
  const full = selected.find(e => e.method === 'thread/tokenUsage/updated'
    && Number.isFinite(e.params.tokenUsage?.modelContextWindow)
    && e.params.tokenUsage.modelContextWindow > 0
    && e.params.tokenUsage.total?.totalTokens === e.params.tokenUsage.modelContextWindow);
  if (!full) errors.push('Codex did not report context-full token state');
  const preceding = events.find(e => e.method === 'turn/completed'
    && e.params?.threadId === meta.threadId && e.params.turn?.id === meta.contextProbeAfterTurnId
    && e.params.turn.status === 'completed' && e.atMs < (request?.atMs ?? -Infinity));
  if (!preceding) errors.push('context probe did not follow the successful long-session work');
  if (selected.some(e => e.method === 'item/completed'
      && ['commandExecution', 'fileChange'].includes(e.params.item?.type)))
    errors.push('context rejection unexpectedly executed tools');
  return { errors, verified: errors.length === 0 };
}

module.exports = { isExpectedContextError, validateContextProbe };
