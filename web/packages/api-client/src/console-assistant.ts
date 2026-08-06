import { apiFetch } from './transport';
import { ApiClientError } from './errors';
import {
  consumeConsoleRuntimeEventStream,
  type ConsoleFlowDebugStreamHandlers
} from './console/application-runtime';

export interface ConsoleAssistantPreference {
  application_id: string | null;
  mcp_instance_ids: string[];
  model?: string | null;
  reasoning_effort?: string | null;
}

export interface ConsoleAssistantRunCapabilities {
  model_selection_enabled: boolean;
  reasoning_effort_enabled: boolean;
  models: Array<{
    id: string;
    name: string | null;
    context_window: number | null;
    reasoning_efforts: string[];
    default_reasoning_effort: string | null;
  }>;
}

export interface ConsoleAssistantSettings {
  preference: ConsoleAssistantPreference;
  published_agent_flows: Array<{ application_id: string; name: string }>;
  enabled_mcp_instances: Array<{ instance_id: string; name: string }>;
  run_capabilities: ConsoleAssistantRunCapabilities;
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
  return apiFetch<ConsoleAssistantSettings>({
    path: '/api/console/assistant/settings'
  });
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

export async function startConsoleAssistantRunStream(
  body: StartConsoleAssistantRunInput,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: { baseUrl?: string }
) {
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);
  const response = await fetch(
    `${options?.baseUrl ?? ''}/api/console/assistant/runs/stream`,
    {
      method: 'POST',
      credentials: 'include',
      signal: abortController.signal,
      headers: {
        accept: 'text/event-stream',
        'content-type': 'application/json',
        'x-csrf-token': csrfToken
      },
      body: JSON.stringify(body)
    }
  );

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  await consumeConsoleRuntimeEventStream(response, handlers);
}
