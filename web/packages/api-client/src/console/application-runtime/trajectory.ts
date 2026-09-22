import { apiFetch } from '../../transport';

export type ProviderTrajectoryView = 'semantic' | 'protocol';

export interface ProviderTrajectoryOptions {
  request_id?: string;
  focus_event_id?: string;
}
export interface ProviderTrajectoryStep {
  links: Array<{
    relation: 'trigger' | 'context';
    flow_run_id: string;
    request_id: string;
    response_id?: string | null;
  }>;
  event_id: string;
  event_sequence: number;
  event_type: 'provider_semantic_step';
  created_at: string;
  metadata: {
    purpose: 'prewarm' | 'generate' | 'tool_resume' | 'compact' | 'unknown';
    run_mode?: string;
    duration_ms?: number | null;
    source: 'ai_native' | 'supplier_protocol';
    protocol?: string;
    transport?: 'http' | 'sse' | 'websocket';
    direction?: 'prepared' | 'received';
    kind:
      | 'model_call'
      | 'model_reply'
      | 'tool_call'
      | 'tool_result'
      | 'error'
      | 'observation_gap';
    status: 'recorded' | 'incomplete' | 'unavailable';
    step_key: string;
    preview?: string;
    tool_call_id?: string;
    provenance?: 'submitted_tool_result' | 'supplier_protocol' | 'ai_native';
    reason?: string;
    flow_run_id: string;
    node_id: string;
    node_run_id: string;
    invocation_id: string;
    provider_attempt_index: number;
    raw_sequence_start?: number;
    raw_sequence_end?: number;
  };
}
export interface ProviderTrajectoryPage {
  items: ProviderTrajectoryStep[];
  next_cursor: number | null;
  observation_count: number;
  persist_failed_count: number;
  protocol_integrity: 'complete' | 'incomplete' | 'pending' | 'not_recorded';
  protocol_persist_failed_count: number;
  integrity:
    | 'complete'
    | 'incomplete'
    | 'pending'
    | 'unavailable'
    | 'not_recorded';
}
export interface ProviderTrajectoryBody {
  sections: Array<{
    kind: 'configuration' | 'system' | 'context' | 'tools' | 'output' | 'error';
    value: unknown;
  }>;
  source: 'ai_native' | 'supplier_protocol';
  evidence_scope: 'step' | 'invocation';
  event_id: string;
  items: Array<{
    event_id: string;
    sequence: number;
    body: string;
    encoding: 'utf8' | 'base64';
  }>;
  next_cursor: number | null;
}
export function getConsoleProviderTrajectory(
  applicationId: string,
  runId: string,
  nodeRunId: string,
  cursor?: number,
  baseUrl?: string,
  options?: ProviderTrajectoryOptions
) {
  const query = new URLSearchParams({ limit: '50' });
  if (options?.request_id) query.set('request_id', options.request_id);
  if (options?.focus_event_id && cursor === undefined)
    query.set('focus_event_id', options.focus_event_id);
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ProviderTrajectoryPage>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/nodes/${nodeRunId}/trajectory?${query}`,
    baseUrl
  });
}
export function getConsoleProviderTrajectoryBody(
  applicationId: string,
  runId: string,
  nodeRunId: string,
  eventId: string,
  cursor?: number,
  view: ProviderTrajectoryView = 'semantic',
  baseUrl?: string
) {
  const query = new URLSearchParams({ limit: '8', view });
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ProviderTrajectoryBody>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/nodes/${nodeRunId}/trajectory/${eventId}?${query}`,
    baseUrl
  });
}

export function getConsoleRunTrajectory(
  applicationId: string,
  runId: string,
  cursor?: number,
  baseUrl?: string,
  options?: ProviderTrajectoryOptions
) {
  const query = new URLSearchParams({ limit: '50' });
  if (options?.request_id) query.set('request_id', options.request_id);
  if (options?.focus_event_id && cursor === undefined)
    query.set('focus_event_id', options.focus_event_id);
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ProviderTrajectoryPage>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/trajectory?${query}`,
    baseUrl
  });
}

export function getConsoleRunPayload(
  applicationId: string,
  runId: string,
  section: 'input_payload' | 'output_payload',
  baseUrl?: string
) {
  return apiFetch<Record<string, unknown>>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/payloads/${section}`,
    baseUrl
  });
}
