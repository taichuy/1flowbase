import { apiFetch } from '../../transport';
export interface ClientTrajectoryStep {
  id: string;
  request_id: string;
  sequence: number;
  created_at: string;
  category: string;
  name: string;
  preview: string;
  parameters_preview: string | null;
  result_preview: string | null;
  status: string;
  origin: string;
  protocol: string;
  transport: 'http' | 'websocket';
  flow_run_id: string;
  node_run_id: string | null;
  parent_id: string | null;
  call_id: string | null;
  item_id: string | null;
  response_id: string | null;
  turn_id: string | null;
  related_step_id: string | null;
  available_sections: string[];
}
export interface ClientTrajectoryPage {
  items: ClientTrajectoryStep[];
  next_cursor: number | null;
  integrity: string;
}
export interface ClientTrajectorySection {
  request_id: string;
  evidence_scope: 'capture' | 'step';
  step_id: string;
  section: string;
  items: Array<{ sequence: number; value: unknown }>;
  next_cursor: number | null;
}
export function getConsoleClientTrajectory(
  applicationId: string,
  runId: string,
  nodeRunId?: string,
  cursor?: number,
  baseUrl?: string
) {
  const query = new URLSearchParams({ limit: '50' });
  if (nodeRunId) query.set('node_run_id', nodeRunId);
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ClientTrajectoryPage>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/client-trajectory?${query}`,
    baseUrl
  });
}
export function getConsoleClientTrajectorySection(
  applicationId: string,
  runId: string,
  stepId: string,
  section: string,
  nodeRunId?: string,
  cursor?: number,
  baseUrl?: string
) {
  const query = new URLSearchParams({ limit: '8', section });
  if (nodeRunId) query.set('node_run_id', nodeRunId);
  if (cursor !== undefined) query.set('cursor', String(cursor));
  return apiFetch<ClientTrajectorySection>({
    path: `/api/console/applications/${applicationId}/logs/runs/${runId}/client-trajectory/${stepId}?${query}`,
    baseUrl
  });
}
