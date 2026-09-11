import { apiFetch } from '../../transport';

export interface GatewayLogQuery {
  conversation_id?: string;
  turn_id?: string;
  flow_run_id?: string;
  page?: number;
  page_size?: number;
}
export interface GatewayLogMetrics {
  invocation_count: number;
  attempt_count: number;
  failed_attempt_count: number;
  input_tokens: number | null;
  output_tokens: number | null;
  total_tokens: number | null;
  elapsed_ms: number | null;
  model_duration_ms: number | null;
  tool_result_wait_ms: number | null;
  costs: { currency_code: string; amount: string }[];
  unknown_cost_attempts: number;
}
export interface GatewayLogMessage {
  id: string; kind: string; phase: string | null; text: string | null;
  call_id: string | null; tool_name: string | null; tool_input: string | null;
  tool_result: string | null; result_received: boolean; execution_verified: boolean;
  flow_run_id: string; sequence: number; identity_status: string;
}
export interface GatewayLogEntry {
  id: string; kind: string; title: string; identity_status: string;
  thread_id: string | null; client_turn_id: string | null; request_kind: string | null;
  parent_thread_id: string | null; parent_turn_id: string | null; relation_status: string;
  parent_task_id: string | null; parent_conversation_id: string | null; forked_from_thread_id: string | null; identity_sources: string[];
  completion_status: string; observations: string[];
  flow_run_id: string | null; caused_by_run_id: string | null;
  started_at: string; finished_at: string | null; status: string | null;
  attempt_index: number | null; is_retry: boolean | null; error_code: string | null;
  metrics: GatewayLogMetrics; messages: GatewayLogMessage[]; messages_has_more: boolean;
}
export interface GatewayLogPage {
  items: GatewayLogEntry[]; total: number; page: number; page_size: number;
}
export function getConsoleGatewayLogs(applicationId: string, query: GatewayLogQuery, baseUrl?: string) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) params.set(key, String(value));
  }
  return apiFetch<GatewayLogPage>({ path: `/api/console/applications/${applicationId}/logs/gateway?${params}`, baseUrl });
}
