import { apiFetch } from './transport';

export interface ConsoleAssistantPreference {
  application_id: string | null;
  mcp_instance_ids: string[];
}

export interface ConsoleAssistantSettings {
  preference: ConsoleAssistantPreference;
  published_agent_flows: Array<{ application_id: string; name: string }>;
  enabled_mcp_instances: Array<{ instance_id: string; name: string }>;
}

export interface StartConsoleAssistantRunInput {
  query: string;
  history: Array<{ role: 'user' | 'assistant'; content: string }>;
  title?: string;
}

export interface ConsoleAssistantRun {
  id: string;
  application_id: string;
  status: string;
  answer: string | null;
  output_payload: unknown;
  error_payload: unknown | null;
}

export function getConsoleAssistantSettings() {
  return apiFetch<ConsoleAssistantSettings>({ path: '/api/console/assistant/settings' });
}

export function updateConsoleAssistantSettings(
  preference: ConsoleAssistantPreference,
  csrfToken: string
) {
  return apiFetch<ConsoleAssistantSettings>({
    path: '/api/console/assistant/settings',
    method: 'PATCH',
    body: preference,
    csrfToken
  });
}

export function startConsoleAssistantRun(
  body: StartConsoleAssistantRunInput,
  csrfToken: string
) {
  return apiFetch<ConsoleAssistantRun>({
    path: '/api/console/assistant/runs',
    method: 'POST',
    body,
    csrfToken
  });
}
