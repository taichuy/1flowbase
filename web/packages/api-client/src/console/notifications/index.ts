import type { ConsolePluginFormSchema } from '../modeling/model-providers';
import { apiFetch, apiFetchVoid, getDefaultApiBaseUrl } from '../../transport';

export interface NotificationAuthorizationDescriptor {
  supports_delivery_window?: boolean;
  actions: Array<{ code: string; label: string; kind: 'qr_code' }>;
  managed_secret_keys: string[];
}
export interface NotificationDeliveryWindowView {
  status: 'unknown' | 'waiting_for_message' | 'active' | 'expired';
  last_received_at_ms?: number;
  expires_at_ms?: number;
}
export interface NotificationAuthorizationView {
  delivery_window?: NotificationDeliveryWindowView;
  status: 'unauthorized' | 'pending' | 'authorized' | 'invalid';
  user_action?: {
    kind: 'qr_code';
    qr_code_url: string;
    poll_interval_seconds: number;
    expires_at?: string;
  };
}
export type NotificationAuthorizationOperation =
  | { type: 'begin'; action: string }
  | { type: 'poll' }
  | { type: 'cancel' }
  | { type: 'maintain' };

export interface NotificationFieldLocale {
  label?: string;
  description?: string;
  placeholder?: string;
}
export interface NotificationFormLocale {
  title?: string;
  description?: string;
  fields?: Record<string, NotificationFieldLocale>;
}
export interface NotificationAuthorizationActionLocale {
  label: string;
}
export interface NotificationAuthorizationLocale {
  actions?: Record<string, NotificationAuthorizationActionLocale>;
}
export interface NotificationTypeLocale {
  display_name?: string;
  description?: string;
  config_form?: NotificationFormLocale;
  input_form?: NotificationFormLocale;
  authorization?: NotificationAuthorizationLocale;
}
export interface NotificationType {
  type_code: string;
  display_name: string;
  description: string | null;
  config_schema: Record<string, unknown>;
  config_form: ConsolePluginFormSchema;
  input_schema: Record<string, unknown>;
  input_form: ConsolePluginFormSchema;
  output_schema: Record<string, unknown>;
  receive?: { default_system_prompt: string; mcp_tools: unknown } | null;
  authorization?: NotificationAuthorizationDescriptor | null;
  i18n?: Partial<Record<'zh_Hans' | 'en_US', NotificationTypeLocale>>;
  contribution_id: string;
  installation_id: string | null;
  available: boolean;
  unavailable_reason: string | null;
}
export interface NotificationInstance {
  id: string;
  workspace_id: string;
  type_code: string;
  installation_id: string | null;
  display_name: string;
  status: 'enabled' | 'disabled';
  authorization: NotificationAuthorizationView | null;
  config_json: Record<string, unknown>;
  configured_secret_fields: string[];
  available: boolean;
  unavailable_reason: string | null;
  created_at: string;
  updated_at: string;
}
export interface NotificationMessage {
  id: string;
  workspace_id: string;
  instance_id: string;
  recipient_user_id: string;
  title: string;
  content: string;
  read_at: string | null;
  created_at: string;
}
export interface NotificationMessages {
  items: NotificationMessage[];
  next_cursor: string | null;
  unread_count: number;
}
export type NotificationReadStatus = 'all' | 'unread' | 'read';
export interface NotificationChannel {
  instance_id: string;
  display_name: string;
  type_code: string;
  deleted: boolean;
  latest_title: string;
  latest_content: string;
  latest_created_at: string;
  unread_count: number;
}
export interface NotificationChannels {
  items: NotificationChannel[];
  next_cursor: string | null;
}
export interface NotificationMessageListInput {
  cursor?: string;
  page_size?: number;
  unread_only?: boolean;
  instance_id?: string;
  read_status?: NotificationReadStatus;
}
export type NotificationLogStatus =
  | 'pending'
  | 'success'
  | 'failure'
  | 'unknown';
export interface NotificationSendLog {
  id: string;
  workspace_id: string;
  actor_user_id: string;
  instance_id: string;
  display_name: string | null;
  type_code: string | null;
  source: 'api' | 'workflow';
  status: NotificationLogStatus;
  failure_code: string | null;
  title: string | null;
  content: string | null;
  recipient_count: number | null;
  created_at: string;
  completed_at: string | null;
}
export interface NotificationLogListInput {
  page?: number;
  page_size?: number;
  instance_id?: string;
  type_code?: string;
  status?: NotificationLogStatus;
  source?: NotificationSendLog['source'];
  created_from?: string;
  created_to?: string;
}
export interface NotificationSendLogs {
  items: NotificationSendLog[];
  total: number;
  page: number;
  page_size: number;
}
export interface CreateNotificationInstance {
  type_code: string;
  installation_id: string | null;
  display_name: string;
  config_json: Record<string, unknown>;
  status?: NotificationInstance['status'];
}
export type UpdateNotificationInstance = Partial<
  Pick<CreateNotificationInstance, 'display_name' | 'status' | 'config_json'>
>;
export interface NotificationInstanceListInput {
  display_name?: string;
  type_code?: string;
  status?: NotificationInstance['status'];
}
const path = '/api/console/notifications';
const managementPath = '/api/console/communications';
export const listNotificationTypes = (signal?: AbortSignal) =>
  apiFetch<NotificationType[]>({ path: `${managementPath}/types`, signal });
export function listNotificationInstances(
  input: NotificationInstanceListInput = {},
  signal?: AbortSignal
) {
  const search = new URLSearchParams();
  if (input.display_name) search.set('display_name', input.display_name);
  if (input.type_code) search.set('type_code', input.type_code);
  if (input.status) search.set('status', input.status);
  const query = search.size ? `?${search}` : '';
  return apiFetch<NotificationInstance[]>({
    path: `${managementPath}/instances${query}`,
    signal
  });
}
export const getNotificationInstance = (id: string, signal?: AbortSignal) =>
  apiFetch<NotificationInstance>({
    path: `${managementPath}/instances/${encodeURIComponent(id)}`,
    signal
  });
export const createNotificationInstance = (
  body: CreateNotificationInstance,
  csrfToken: string
) =>
  apiFetch<NotificationInstance>({
    path: `${managementPath}/instances`,
    method: 'POST',
    body,
    csrfToken
  });
export const updateNotificationInstance = (
  id: string,
  body: UpdateNotificationInstance,
  csrfToken: string
) =>
  apiFetch<NotificationInstance>({
    path: `${managementPath}/instances/${encodeURIComponent(id)}`,
    method: 'PATCH',
    body,
    csrfToken
  });
export const authorizeNotificationInstance = (
  id: string,
  body: NotificationAuthorizationOperation,
  csrfToken: string,
  signal?: AbortSignal
) =>
  apiFetch<NotificationAuthorizationView>({
    path: `${managementPath}/instances/${encodeURIComponent(id)}/authorization`,
    method: 'POST',
    body,
    csrfToken,
    signal
  });
export const deleteNotificationInstance = (id: string, csrfToken: string) =>
  apiFetchVoid({
    path: `${managementPath}/instances/${encodeURIComponent(id)}`,
    method: 'DELETE',
    csrfToken
  });
export const sendNotification = (
  id: string,
  input: Record<string, unknown>,
  csrfToken: string
) =>
  apiFetch<{
    instance_id: string;
    type_code: string;
    result: Record<string, unknown>;
  }>({
    path: `${managementPath}/instances/${encodeURIComponent(id)}/send`,
    method: 'POST',
    body: { input },
    csrfToken
  });
export function listNotificationMessages(
  input: NotificationMessageListInput = {},
  signal?: AbortSignal
) {
  const search = new URLSearchParams();
  if (input.cursor) search.set('cursor', input.cursor);
  if (input.page_size !== undefined)
    search.set('page_size', String(input.page_size));
  if (input.unread_only !== undefined)
    search.set('unread_only', String(input.unread_only));
  if (input.instance_id) search.set('instance_id', input.instance_id);
  if (input.read_status) search.set('read_status', input.read_status);
  return apiFetch<NotificationMessages>({
    path: `${path}/messages?${search}`,
    signal
  });
}
export const getNotificationUnreadCount = (signal?: AbortSignal) =>
  apiFetch<{ unread_count: number }>({
    path: `${path}/messages/unread-count`,
    signal
  });
export const markNotificationRead = (id: string, csrfToken: string) =>
  apiFetch<NotificationMessage>({
    path: `${path}/messages/${encodeURIComponent(id)}/read`,
    method: 'POST',
    csrfToken
  });
export const markAllNotificationsRead = (csrfToken: string) =>
  apiFetch<{ updated_count: number }>({
    path: `${path}/messages/read-all`,
    method: 'POST',
    csrfToken
  });
export const markNotificationUnread = (id: string, csrfToken: string) =>
  apiFetch<NotificationMessage>({
    path: `${path}/messages/${encodeURIComponent(id)}/unread`,
    method: 'POST',
    csrfToken
  });
export const markNotificationChannelRead = (
  instance_id: string,
  csrfToken: string
) =>
  apiFetch<{ updated_count: number }>({
    path: `${path}/channels/${encodeURIComponent(instance_id)}/read-all`,
    method: 'POST',
    csrfToken
  });
export function listNotificationChannels(
  input: {
    cursor?: string;
    page_size?: number;
    read_status?: NotificationReadStatus;
  } = {},
  signal?: AbortSignal
) {
  const search = new URLSearchParams();
  if (input.cursor) search.set('cursor', input.cursor);
  if (input.page_size !== undefined)
    search.set('page_size', String(input.page_size));
  if (input.read_status) search.set('read_status', input.read_status);
  return apiFetch<NotificationChannels>({
    path: `${path}/channels?${search}`,
    signal
  });
}
export function listNotificationSendLogs(
  input: NotificationLogListInput = {},
  signal?: AbortSignal
) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(input)) {
    if (value !== undefined && value !== '') search.set(key, String(value));
  }
  return apiFetch<NotificationSendLogs>({
    path: `${managementPath}/logs?${search}`,
    signal
  });
}
export const getNotificationSendLog = (id: string, signal?: AbortSignal) =>
  apiFetch<NotificationSendLog>({
    path: `${managementPath}/logs/${encodeURIComponent(id)}`,
    signal
  });
/** The session cookie is the same credential used by apiFetch. Every open,
 * including reconnect, requests a fresh HTTP snapshot from the consumer. */
export function subscribeNotificationChanges(
  onRefresh: () => void,
  onConnectionChange: (connected: boolean) => void,
  baseUrl = getDefaultApiBaseUrl()
) {
  const source = new EventSource(`${baseUrl}${path}/events`, {
    withCredentials: true
  });
  source.onopen = () => {
    onConnectionChange(true);
    onRefresh();
  };
  source.onerror = () => onConnectionChange(false);
  source.addEventListener('notification.changed', onRefresh);
  return () => source.close();
}

export * from './communications';
