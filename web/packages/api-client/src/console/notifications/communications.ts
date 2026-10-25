import { apiFetch } from '../../transport';
import type { ConsoleAssistantRunCapabilities } from '../administration/assistant';
export interface CommunicationBindingInput {
  application_id: string;
  model: string | null;
  reasoning_effort: string | null;
  file_table_id: string | null;
  mcp_instance_ids: string[];
  system_prompt: string;
}
export interface CommunicationBindingRecord extends CommunicationBindingInput {
  workspace_id: string;
  instance_id: string;
  actor_user_id: string;
  plugin_mcp_instance_id: string;
  revision: number;
}
export interface CommunicationBindingView {
  binding: CommunicationBindingRecord | null;
  plugin_mcp_instance_id: string | null;
  default_system_prompt: string | null;
}
export interface CommunicationOptions {
  published_agent_flows: Array<{ application_id: string; name: string }>;
  enabled_mcp_instances: Array<{ instance_id: string; name: string }>;
  run_capabilities: ConsoleAssistantRunCapabilities;
}
export interface CommunicationPage<T> {
  items: T[];
  next_cursor: string | null;
}
export interface CommunicationConversation {
  id: string;
  workspace_id: string;
  instance_id: string;
  external_identity: string;
  external_conversation_key: string;
  sender_display_name: string | null;
}
export interface CommunicationMessage {
  id: string;
  conversation_id: string;
  external_message_key: string;
  received_at_ms: number;
  content: string | null;
  processing_status: 'pending' | 'processing' | 'succeeded' | 'failed';
  run_id: string | null;
  final_reply: string | null;
  send_status:
    | 'none'
    | 'pending'
    | 'sending'
    | 'succeeded'
    | 'failed'
    | 'unknown';
  failure_code: string | null;
  can_retry: boolean;
}
export interface CommunicationAttachment {
  id: string;
  message_id: string;
  provider_attachment_id: string;
  file_name: string;
  media_type: string;
  size_bytes: number | null;
  status: 'pending' | 'ready' | 'failed';
  file_table_id: string | null;
  file_record_id: string | null;
  failure_code: string | null;
}
const path = '/api/console/communications';
const instancePath = (id: string) =>
  `${path}/instances/${encodeURIComponent(id)}`;
export const getCommunicationOptions = (
  application_id?: string,
  signal?: AbortSignal
) =>
  apiFetch<CommunicationOptions>({
    path: `${path}/options${application_id ? `?${new URLSearchParams({ application_id })}` : ''}`,
    signal
  });
export const getCommunicationBinding = (id: string, signal?: AbortSignal) =>
  apiFetch<CommunicationBindingView>({
    path: `${instancePath(id)}/binding`,
    signal
  });
export const putCommunicationBinding = (
  id: string,
  body: CommunicationBindingInput,
  csrfToken: string
) =>
  apiFetch<CommunicationBindingRecord>({
    path: `${instancePath(id)}/binding`,
    method: 'PUT',
    body,
    csrfToken
  });
export interface CommunicationPageInput {
  cursor?: string;
  limit?: number;
}
const query = (input: CommunicationPageInput) =>
  new URLSearchParams(Object.entries(input).map(([k, v]) => [k, String(v)]));
export const listCommunicationConversations = (
  id: string,
  input: CommunicationPageInput = {},
  signal?: AbortSignal
) =>
  apiFetch<CommunicationPage<CommunicationConversation>>({
    path: `${instancePath(id)}/conversations?${query(input)}`,
    signal
  });
export const listCommunicationMessages = (
  id: string,
  conversation_id: string,
  input: CommunicationPageInput = {},
  signal?: AbortSignal
) =>
  apiFetch<CommunicationPage<CommunicationMessage>>({
    path: `${instancePath(id)}/conversations/${encodeURIComponent(conversation_id)}/messages?${query(input)}`,
    signal
  });
export const listCommunicationFiles = (
  id: string,
  conversation_id: string,
  input: CommunicationPageInput = {},
  signal?: AbortSignal
) =>
  apiFetch<CommunicationPage<CommunicationAttachment>>({
    path: `${instancePath(id)}/conversations/${encodeURIComponent(conversation_id)}/files?${query(input)}`,
    signal
  });
export const retryCommunicationMessage = (
  id: string,
  message_id: string,
  csrfToken: string
) =>
  apiFetch<CommunicationMessage>({
    path: `${instancePath(id)}/messages/${encodeURIComponent(message_id)}/retry`,
    method: 'POST',
    csrfToken
  });

export interface CommunicationFileRead {
  attachment: CommunicationAttachment;
  file_name: string;
  content_type: string;
  content_length: number;
  offset: number;
  next_offset: number | null;
  eof: boolean;
  content?: string | null;
  data_base64: string;
  url: string | null;
  preview_url: string | null;
}
export const getCommunicationFile = (
  id: string,
  attachment_id: string,
  signal?: AbortSignal
) =>
  apiFetch<CommunicationFileRead>({
    path: `${instancePath(id)}/files/${encodeURIComponent(attachment_id)}`,
    signal
  });

/** Workspace management projections; scoped channel history remains a separate API. */
export interface CommunicationManagementPage<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}
export interface CommunicationMessageListItem extends Pick<
  CommunicationMessage,
  | 'id'
  | 'conversation_id'
  | 'received_at_ms'
  | 'processing_status'
  | 'send_status'
  | 'failure_code'
  | 'can_retry'
  | 'run_id'
> {
  instance_id: string;
  instance_display_name: string | null;
  external_identity: string;
  sender_display_name: string | null;
  application_id: string | null;
  content_preview: string | null;
}
export interface CommunicationConversationListItem extends CommunicationConversation {
  instance_display_name: string | null;
  latest_message: CommunicationMessageListItem | null;
}
export interface CommunicationMessageDetail extends CommunicationMessage {
  instance_id: string;
  instance_display_name: string | null;
  external_identity: string;
  sender_display_name: string | null;
  application_id: string | null;
  attachments: CommunicationAttachment[];
}
export interface CommunicationConversationListInput {
  instance_id?: string;
  conversation_id?: string;
  page?: number;
  page_size?: number;
}
export interface CommunicationMessageListInput extends CommunicationConversationListInput {
  processing_status?: CommunicationMessage['processing_status'];
  send_status?: CommunicationMessage['send_status'];
}
const managementQuery = (input: CommunicationMessageListInput) =>
  new URLSearchParams(
    Object.entries(input)
      .filter(([, value]) => value !== undefined)
      .map(([key, value]) => [key, String(value)])
  );
export const listCommunicationConversationOverview = (
  input: CommunicationConversationListInput = {},
  signal?: AbortSignal
) =>
  apiFetch<CommunicationManagementPage<CommunicationConversationListItem>>({
    path: `${path}/conversations?${managementQuery(input)}`,
    signal
  });
export const listCommunicationMessageOverview = (
  input: CommunicationMessageListInput = {},
  signal?: AbortSignal
) =>
  apiFetch<CommunicationManagementPage<CommunicationMessageListItem>>({
    path: `${path}/messages?${managementQuery(input)}`,
    signal
  });
export const getCommunicationMessageDetail = (
  message_id: string,
  signal?: AbortSignal
) =>
  apiFetch<CommunicationMessageDetail>({
    path: `${path}/messages/${encodeURIComponent(message_id)}`,
    signal
  });
