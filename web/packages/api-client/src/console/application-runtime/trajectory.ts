import { apiFetch } from '../../transport';

export interface ProviderTrajectoryStep {
  event_id: string;
  event_sequence: number;
  event_type: 'provider_protocol_observation';
  created_at: string;
  metadata: {
    protocol: string;
    transport: 'http' | 'sse' | 'websocket';
    direction: 'sent' | 'received';
    kind:
      | 'request'
      | 'response_head'
      | 'response_body'
      | 'message'
      | 'stream_end';
    encoding: 'utf8' | 'base64';
    status?: number;
    flow_run_id: string;
    node_id: string;
    node_run_id: string;
    invocation_id: string;
    provider_attempt_index: number;
    sequence: number;
  };
}
export interface ProviderTrajectoryPage {
  items: ProviderTrajectoryStep[];
  next_cursor: number | null;
  observation_count: number;
  persist_failed_count: number;
  integrity: 'complete' | 'incomplete' | 'unavailable';
}
export interface ProviderTrajectoryBody {
  event_id: string;
  body: string;
  encoding: 'utf8' | 'base64';
}
export function getConsoleProviderTrajectory(
  applicationId: string,
  runId: string,
  nodeRunId: string,
  cursor?: number,
  baseUrl?: string
) {
  const query = new URLSearchParams({ limit: '50' });
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
  baseUrl?: string
) {
  return apiFetch<ProviderTrajectoryBody>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/nodes/${nodeRunId}/trajectory/${eventId}`,
    baseUrl
  });
}
