import { useSyncExternalStore } from 'react';
import type { CommunicationMessage } from '@1flowbase/api-client';

export interface CommunicationLocation {
  tab: 'channels' | 'conversations' | 'logs';
  record_tab: 'received' | 'sent';
  instance_id?: string;
  processing_status?: NonNullable<CommunicationMessage['processing_status']>;
  send_status?: NonNullable<CommunicationMessage['send_status']>;
  page: number;
  conversation_id?: string;
}
const processingStatuses = [
  'pending',
  'processing',
  'succeeded',
  'failed'
] as const;
const sendStatuses = [
  'none',
  'pending',
  'sending',
  'succeeded',
  'failed',
  'unknown'
] as const;
function positivePage(value: string | null) {
  const page = Number(value);
  return Number.isSafeInteger(page) && page > 0 ? page : 1;
}
function readCommunicationLocation(
  search: string
): CommunicationLocation {
  const params = new URLSearchParams(search);
  const processing = params.get('processing_status');
  const send = params.get('send_status');
  return {
    tab:
      params.get('tab') === 'logs'
        ? 'logs'
        : params.get('tab') === 'conversations'
          ? 'conversations'
          : 'channels',
    record_tab: params.get('record_tab') === 'sent' ? 'sent' : 'received',
    instance_id: params.get('instance_id') || undefined,
    processing_status: processingStatuses.find((value) => value === processing),
    send_status: sendStatuses.find((value) => value === send),
    page: positivePage(params.get('page')),
    conversation_id: params.get('conversation_id') || undefined
  };
}
function subscribe(listener: () => void) {
  window.addEventListener('popstate', listener);
  return () => window.removeEventListener('popstate', listener);
}
export function useCommunicationLocation() {
  const search = useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => ''
  );
  return readCommunicationLocation(search);
}
function communicationHref(patch: Partial<CommunicationLocation>) {
  const url = new URL(window.location.href);
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) url.searchParams.delete(key);
    else url.searchParams.set(key, String(value));
  }
  return `${url.pathname}${url.search}${url.hash}`;
}
export function navigateCommunication(patch: Partial<CommunicationLocation>) {
  const href = communicationHref(patch);
  if (
    href ===
    `${window.location.pathname}${window.location.search}${window.location.hash}`
  )
    return;
  window.history.pushState(window.history.state, '', href);
  window.dispatchEvent(new PopStateEvent('popstate'));
}
export function communicationRunHref(item: {
  application_id: string | null;
  run_id: string | null;
}) {
  return item.application_id && item.run_id
    ? `/applications/${encodeURIComponent(item.application_id)}/logs?${new URLSearchParams({ run_id: item.run_id, view: 'trace' })}`
    : null;
}
