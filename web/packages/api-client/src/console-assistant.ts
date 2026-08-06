import { apiFetch } from './transport';
import { ApiClientError } from './errors';
import {
  consumeConsoleRuntimeEventStream,
  normalizeConsoleRuntimeEvent,
  type ConsoleFlowDebugStreamHandlers
} from './console/application-runtime';
import { getDefaultApiBaseUrl } from './transport';

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
  application_id: string;
  query: string;
  history: Array<{ role: 'user' | 'assistant'; content: string }>;
  title?: string;
}

interface ConsoleAssistantWebSocketTicket {
  ticket: string;
  protocol: string;
  expires_in_seconds: number;
}

export interface ConsoleAssistantWebSocketControl {
  cancel(runId: string): void;
  close(): void;
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

export async function startConsoleAssistantRunWebSocket(
  body: StartConsoleAssistantRunInput,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: {
    baseUrl?: string;
    onControl?: (control: ConsoleAssistantWebSocketControl) => void;
    maxReconnects?: number;
  }
) {
  const baseUrl = options?.baseUrl ?? getDefaultApiBaseUrl();
  const maxReconnects = options?.maxReconnects ?? 2;
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);

  await new Promise<void>((resolve, reject) => {
    let socket: WebSocket | null = null;
    let activeRunId: string | null = null;
    let lastEventId: string | null = null;
    let terminal = false;
    let reconnectCount = 0;
    let settled = false;
    let requestSequence = 1;

    const finish = (error?: unknown) => {
      if (settled) {
        return;
      }
      settled = true;
      socket?.close();
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    };

    const control: ConsoleAssistantWebSocketControl = {
      cancel(runId) {
        if (socket?.readyState === WebSocket.OPEN) {
          socket.send(
            JSON.stringify({
              type: 'run.cancel',
              request_id: `cancel-${requestSequence++}`,
              run_id: runId
            })
          );
        }
      },
      close() {
        finish(new DOMException('Aborted', 'AbortError'));
      }
    };
    options?.onControl?.(control);

    abortController.signal.addEventListener(
      'abort',
      () => finish(new DOMException('Aborted', 'AbortError')),
      { once: true }
    );

    const connect = async (attach: boolean) => {
      try {
        const ticket = await apiFetch<ConsoleAssistantWebSocketTicket>({
          path: '/api/console/assistant/runs/websocket-ticket',
          method: 'POST',
          body: { application_id: body.application_id },
          csrfToken,
          baseUrl
        });
        if (settled) {
          return;
        }
        const url = new URL(
          '/api/console/assistant/runs/websocket',
          baseUrl || getDefaultApiBaseUrl()
        );
        url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
        const current = new WebSocket(url, [
          ticket.protocol,
          `1flowbase.assistant.ticket.${ticket.ticket}`
        ]);
        socket = current;
        current.onopen = () => {
          if (attach && activeRunId) {
            current.send(
              JSON.stringify({
                type: 'run.attach',
                request_id: `attach-${requestSequence++}`,
                run_id: activeRunId,
                after_event_id: lastEventId
              })
            );
            return;
          }
          current.send(
            JSON.stringify({
              type: 'run.create',
              request_id: `create-${requestSequence++}`,
              request: body
            })
          );
        };
        current.onmessage = (message) => {
          let raw: unknown;
          try {
            raw = JSON.parse(String(message.data));
          } catch {
            finish(new Error('Invalid Assistant WebSocket event'));
            return;
          }
          if (
            raw &&
            typeof raw === 'object' &&
            !Array.isArray(raw) &&
            (raw as { type?: unknown }).type === 'error'
          ) {
            const error = (raw as { error?: { message?: unknown } }).error;
            finish(
              new Error(
                typeof error?.message === 'string'
                  ? error.message
                  : 'Assistant WebSocket command failed'
              )
            );
            return;
          }
          const event = normalizeConsoleRuntimeEvent(raw);
          if (!event) {
            return;
          }
          if ('run_id' in event && typeof event.run_id === 'string') {
            activeRunId = event.run_id;
          }
          if ('event_id' in event && typeof event.event_id === 'string') {
            lastEventId = event.event_id;
          }
          handlers.onEvent(event);
          if (
            [
              'flow_finished',
              'flow_incomplete',
              'flow_failed',
              'flow_cancelled',
              'waiting_human',
              'replay_expired',
              'replay_gap'
            ].includes(event.type)
          ) {
            terminal = true;
            finish();
          }
        };
        current.onerror = () => {
          current.close();
        };
        current.onclose = () => {
          if (settled || terminal) {
            return;
          }
          if (activeRunId && reconnectCount < maxReconnects) {
            reconnectCount += 1;
            void connect(true);
            return;
          }
          finish(new Error('Assistant WebSocket connection interrupted'));
        };
      } catch (error) {
        finish(error);
      }
    };

    void connect(false);
  });
}
