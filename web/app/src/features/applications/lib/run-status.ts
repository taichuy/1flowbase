const ACTIVE_RUN_STATUSES = new Set(['queued', 'running', 'paused']);

export function isActiveRunStatus(status: string | null | undefined) {
  return ACTIVE_RUN_STATUSES.has(status?.trim().toLowerCase() ?? '');
}
