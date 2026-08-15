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
  enabled_client_tools: ConsoleAssistantClientToolId[];
}

export type ConsoleAssistantClientToolId =
  | 'get_client_context'
  | 'refresh_client_view';

export interface ConsoleAssistantClientToolCall {
  call_id: string;
  name: ConsoleAssistantClientToolId;
  arguments: Record<string, unknown>;
}

export interface ConsoleAssistantClientToolExecution {
  result: unknown;
  is_error: boolean;
}

export interface ConsoleAssistantClientTools {
  toolIds: ConsoleAssistantClientToolId[];
  execute(
    call: ConsoleAssistantClientToolCall
  ): Promise<ConsoleAssistantClientToolExecution>;
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
  page_reference_max_bytes: number;
  page_reference_max_count: number;
  page_reference_max_total_bytes: number;
  run_capabilities: ConsoleAssistantRunCapabilities;
}

export interface ConsoleAssistantPageReference {
  page_url: string;
  page_title: string;
  outer_html: string;
}

export interface StartConsoleAssistantRunInput {
  application_id: string;
  conversation_id?: string;
  query: string;
  history: Array<{ role: 'user' | 'assistant'; content: string }>;
  page_references?: ConsoleAssistantPageReference[];
  title?: string;
}

export interface ConsoleAssistantConversation {
  conversation_id: string;
  application_id: string;
  created_at: string;
  updated_at: string;
}

export interface CreateConsoleAssistantConversationInput {
  application_id: string;
  seed_legacy_flow_run_id?: string;
}

export interface ConsoleAssistantConversationSummary {
  conversation_id: string | null;
  legacy_flow_run_id: string | null;
  latest_flow_run_id: string | null;
  latest_flow_run_status: string | null;
  title: string | null;
  created_at: string;
  updated_at: string;
}

export interface ConsoleAssistantConversationPage {
  items: ConsoleAssistantConversationSummary[];
  total: number;
  page: number;
  page_size: number;
}

export interface ConsoleAssistantConversationStreamHandlers {
  onSnapshot(page: ConsoleAssistantConversationPage): void;
  onConversation(
    item: ConsoleAssistantConversationSummary,
    eventType: 'conversation.created' | 'conversation.updated'
  ): void;
  getAbortController?(abortController: AbortController): void;
}

export interface ConsoleAssistantConversationMessage {
  id: string;
  flow_run_id: string;
  role: 'user' | 'assistant';
  content: string;
  status: string;
  page_references: ConsoleAssistantPageReference[];
  created_at: string;
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

export function createConsoleAssistantConversation(
  body: CreateConsoleAssistantConversationInput,
  csrfToken: string
) {
  return apiFetch<ConsoleAssistantConversation>({
    path: '/api/console/assistant/conversations',
    method: 'POST',
    body,
    csrfToken
  });
}

export function listConsoleAssistantConversations(
  applicationId: string,
  input: { page?: number; pageSize?: number } = {}
) {
  const search = new URLSearchParams({
    application_id: applicationId,
    page: String(input.page ?? 1),
    page_size: String(input.pageSize ?? 20)
  });
  return apiFetch<ConsoleAssistantConversationPage>({
    path: `/api/console/assistant/conversations?${search.toString()}`
  });
}

export function getConsoleAssistantConversationMessages(
  applicationId: string,
  conversationId: string
) {
  const search = new URLSearchParams({ application_id: applicationId });
  return apiFetch<ConsoleAssistantConversationMessage[]>({
    path: `/api/console/assistant/conversations/${conversationId}/messages?${search.toString()}`
  });
}

export function getConsoleAssistantLegacySnapshotMessages(
  applicationId: string,
  flowRunId: string
) {
  const search = new URLSearchParams({ application_id: applicationId });
  return apiFetch<ConsoleAssistantConversationMessage[]>({
    path: `/api/console/assistant/legacy-runs/${flowRunId}/messages?${search.toString()}`
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

interface ConsoleAssistantWebSocketOptions {
  baseUrl?: string;
  handshakeTimeoutMs?: number;
  onControl?: (control: ConsoleAssistantWebSocketControl) => void;
  maxReconnects?: number;
  clientTools?: ConsoleAssistantClientTools;
}

interface ConsoleAssistantConversationWebSocketOptions {
  baseUrl?: string;
  handshakeTimeoutMs?: number;
  maxReconnects?: number;
}

export async function subscribeConsoleAssistantConversationsWebSocket(
  applicationId: string,
  csrfToken: string,
  handlers: ConsoleAssistantConversationStreamHandlers,
  options?: ConsoleAssistantConversationWebSocketOptions
) {
  const baseUrl = options?.baseUrl ?? getDefaultApiBaseUrl();
  const handshakeTimeoutMs = options?.handshakeTimeoutMs ?? 10_000;
  const maxReconnects = options?.maxReconnects ?? 5;
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);

  await new Promise<void>((resolve, reject) => {
    let socket: WebSocket | null = null;
    let reconnectCount = 0;
    let settled = false;
    let requestSequence = 1;
    let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    const clearTimers = () => {
      if (handshakeTimer !== null) {
        clearTimeout(handshakeTimer);
        handshakeTimer = null;
      }
      if (reconnectTimer !== null) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
    };
    const finish = (error?: unknown) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimers();
      socket?.close();
      error ? reject(error) : resolve();
    };

    abortController.signal.addEventListener('abort', () => finish(), {
      once: true
    });

    const connect = async () => {
      if (settled) {
        return;
      }
      handshakeTimer = setTimeout(
        () =>
          finish(
            new Error('Assistant conversation WebSocket handshake timed out')
          ),
        handshakeTimeoutMs
      );
      try {
        const ticket = await apiFetch<ConsoleAssistantWebSocketTicket>({
          path: '/api/console/assistant/runs/websocket-ticket',
          method: 'POST',
          body: { application_id: applicationId },
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
          current.send(
            JSON.stringify({
              type: 'conversation.subscribe',
              request_id: `conversation-subscribe-${requestSequence++}`
            })
          );
        };
        current.onmessage = (message) => {
          let raw: unknown;
          try {
            raw = JSON.parse(String(message.data));
          } catch {
            finish(new Error('Invalid Assistant conversation WebSocket event'));
            return;
          }
          if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
            return;
          }
          const frame = raw as Record<string, unknown>;
          if (frame.type === 'error') {
            const error = frame.error as { message?: unknown } | undefined;
            finish(
              new Error(
                typeof error?.message === 'string'
                  ? error.message
                  : 'Assistant conversation WebSocket command failed'
              )
            );
            return;
          }
          if (frame.type === 'conversation.snapshot') {
            if (!isConsoleAssistantConversationPage(frame.data)) {
              finish(new Error('Invalid Assistant conversation snapshot'));
              return;
            }
            if (handshakeTimer !== null) {
              clearTimeout(handshakeTimer);
              handshakeTimer = null;
            }
            reconnectCount = 0;
            handlers.onSnapshot(frame.data);
            return;
          }
          if (
            frame.type === 'conversation.created' ||
            frame.type === 'conversation.updated'
          ) {
            if (!isConsoleAssistantConversationSummary(frame.item)) {
              finish(new Error('Invalid Assistant conversation update'));
              return;
            }
            handlers.onConversation(frame.item, frame.type);
          }
        };
        current.onerror = () => current.close();
        current.onclose = () => {
          if (settled) {
            return;
          }
          if (handshakeTimer !== null) {
            clearTimeout(handshakeTimer);
            handshakeTimer = null;
          }
          if (reconnectCount >= maxReconnects) {
            finish(
              new Error(
                'Assistant conversation WebSocket connection interrupted'
              )
            );
            return;
          }
          reconnectCount += 1;
          reconnectTimer = setTimeout(
            () => void connect(),
            Math.min(250 * 2 ** (reconnectCount - 1), 2_000)
          );
        };
      } catch (error) {
        finish(error);
      }
    };

    void connect();
  });
}

function isConsoleAssistantConversationSummary(
  value: unknown
): value is ConsoleAssistantConversationSummary {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return false;
  }
  const item = value as Record<string, unknown>;
  return (
    (typeof item.conversation_id === 'string' ||
      item.conversation_id === null) &&
    (typeof item.legacy_flow_run_id === 'string' ||
      item.legacy_flow_run_id === null) &&
    (typeof item.latest_flow_run_id === 'string' ||
      item.latest_flow_run_id === null) &&
    (typeof item.latest_flow_run_status === 'string' ||
      item.latest_flow_run_status === null) &&
    (typeof item.title === 'string' || item.title === null) &&
    typeof item.created_at === 'string' &&
    typeof item.updated_at === 'string'
  );
}

function isConsoleAssistantConversationPage(
  value: unknown
): value is ConsoleAssistantConversationPage {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return false;
  }
  const page = value as Record<string, unknown>;
  return (
    Array.isArray(page.items) &&
    page.items.every(isConsoleAssistantConversationSummary) &&
    typeof page.total === 'number' &&
    typeof page.page === 'number' &&
    typeof page.page_size === 'number'
  );
}

type ConsoleAssistantWebSocketCommand =
  | { kind: 'create'; body: StartConsoleAssistantRunInput }
  | {
      kind: 'attach';
      applicationId: string;
      runId: string;
      afterEventId?: string | null;
    };

export function startConsoleAssistantRunWebSocket(
  body: StartConsoleAssistantRunInput,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: ConsoleAssistantWebSocketOptions
) {
  return runConsoleAssistantWebSocket(
    { kind: 'create', body },
    csrfToken,
    handlers,
    options
  );
}

export function attachConsoleAssistantRunWebSocket(
  applicationId: string,
  runId: string,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: Omit<ConsoleAssistantWebSocketOptions, 'clientTools'> & {
    afterEventId?: string | null;
  }
) {
  return runConsoleAssistantWebSocket(
    {
      kind: 'attach',
      applicationId,
      runId,
      afterEventId: options?.afterEventId
    },
    csrfToken,
    handlers,
    options
  );
}

async function runConsoleAssistantWebSocket(
  command: ConsoleAssistantWebSocketCommand,
  csrfToken: string,
  handlers: ConsoleFlowDebugStreamHandlers,
  options?: ConsoleAssistantWebSocketOptions
) {
  const baseUrl = options?.baseUrl ?? getDefaultApiBaseUrl();
  const handshakeTimeoutMs = options?.handshakeTimeoutMs ?? 10_000;
  const maxReconnects = options?.maxReconnects ?? 2;
  const abortController = new AbortController();
  handlers.getAbortController?.(abortController);

  await new Promise<void>((resolve, reject) => {
    let socket: WebSocket | null = null;
    let activeRunId: string | null =
      command.kind === 'attach' ? command.runId : null;
    let lastEventId: string | null =
      command.kind === 'attach' ? (command.afterEventId ?? null) : null;
    let terminal = false;
    let reconnectCount = 0;
    let settled = false;
    let requestSequence = 1;
    let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
    const clientToolResults = new Map<string, string>();

    const clearHandshakeDeadline = () => {
      if (handshakeTimer !== null) {
        clearTimeout(handshakeTimer);
        handshakeTimer = null;
      }
    };

    const finish = (error?: unknown) => {
      if (settled) {
        return;
      }
      settled = true;
      clearHandshakeDeadline();
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
      clearHandshakeDeadline();
      handshakeTimer = setTimeout(
        () => finish(new Error('Assistant WebSocket handshake timed out')),
        handshakeTimeoutMs
      );
      try {
        const ticket = await apiFetch<ConsoleAssistantWebSocketTicket>({
          path: '/api/console/assistant/runs/websocket-ticket',
          method: 'POST',
          body: {
            application_id:
              command.kind === 'create'
                ? command.body.application_id
                : command.applicationId
          },
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
          clearHandshakeDeadline();
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
          if (command.kind !== 'create') {
            finish(new Error('Assistant run attach is missing a run ID'));
            return;
          }
          current.send(
            JSON.stringify({
              type: 'run.create',
              request_id: `create-${requestSequence++}`,
              client_tool_ids: options?.clientTools?.toolIds ?? [],
              request: command.body
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
            (raw as { type?: unknown }).type === 'client_tool.call'
          ) {
            const call = raw as {
              call_id?: unknown;
              name?: unknown;
              arguments?: unknown;
            };
            const clientTools = options?.clientTools;
            if (
              !clientTools ||
              typeof call.call_id !== 'string' ||
              (call.name !== 'get_client_context' &&
                call.name !== 'refresh_client_view') ||
              !call.arguments ||
              typeof call.arguments !== 'object' ||
              Array.isArray(call.arguments)
            ) {
              return;
            }
            const cached = clientToolResults.get(call.call_id);
            if (cached) {
              current.send(cached);
              return;
            }
            void clientTools
              .execute({
                call_id: call.call_id,
                name: call.name,
                arguments: call.arguments as Record<string, unknown>
              })
              .then(
                (execution) => execution,
                () => ({
                  result: {
                    status: 'failed',
                    code: 'client_tool_execution_failed'
                  },
                  is_error: true
                })
              )
              .then((execution) => {
                if (current.readyState !== WebSocket.OPEN) {
                  return;
                }
                const result = JSON.stringify({
                  type: 'client_tool.result',
                  request_id: `client-tool-${requestSequence++}`,
                  call_id: call.call_id,
                  result: execution.result,
                  is_error: execution.is_error
                });
                clientToolResults.set(call.call_id as string, result);
                current.send(result);
              });
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
          clearHandshakeDeadline();
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
        clearHandshakeDeadline();
        finish(error);
      }
    };

    void connect(command.kind === 'attach');
  });
}
