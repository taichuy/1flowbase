import type {
  NotificationInstance,
  NotificationMessage,
  NotificationSendLog,
  NotificationType
} from '@1flowbase/api-client';
import {
  commandBindingInstance,
  commandBindingType,
  createStyleBoundaryCommandBindingResponse
} from './command-binding';

const notificationType: NotificationType = {
  type_code: 'system',
  display_name: '系统站内通知',
  description: null,
  config_schema: {},
  config_form: { schema_version: '1.0.0', fields: [] },
  input_schema: {},
  input_form: { schema_version: '1.0.0', fields: [] },
  output_schema: {},
  contribution_id: 'system',
  installation_id: null,
  available: true,
  unavailable_reason: null
};
const notificationInstance: NotificationInstance = {
  id: 'notification-instance',
  workspace_id: 'workspace-1',
  type_code: 'system',
  installation_id: null,
  display_name: '团队工作流通知渠道',
  status: 'enabled',
  authorization: null,
  config_json: {},
  configured_secret_fields: [],
  available: true,
  unavailable_reason: null,
  created_at: '2026-10-13T00:00:00Z',
  updated_at: '2026-10-13T00:00:00Z'
};
const notificationLog: NotificationSendLog = {
  id: 'log-1',
  workspace_id: 'workspace-1',
  actor_user_id: 'user-1',
  instance_id: notificationInstance.id,
  display_name: notificationInstance.display_name,
  type_code: 'system',
  source: 'workflow',
  status: 'success',
  failure_code: null,
  title: '工作流运行完成',
  content: `${'工作流已成功运行，完整处理结果已保存。\n'.repeat(30)}结果地址：https://example.com/${'notification-result-'.repeat(12)}`,
  extra: {
    accepted: true,
    delivery_id: 'provider-receipt-20261027',
    recipients: ['团队成员甲', '团队成员乙'],
    delivery: {
      attempts: 1,
      retry_required: false,
      receipt_url: `https://example.com/${'delivery-receipt-'.repeat(12)}`
    }
  },
  recipient_count: 2,
  created_at: '2026-10-13T00:00:00Z',
  completed_at: '2026-10-13T00:01:00Z'
};

export function seedStyleBoundaryNotificationSettingsFetch() {
  const originalFetch = globalThis.fetch.bind(globalThis);
  const commandBindingResponse = createStyleBoundaryCommandBindingResponse();
  globalThis.fetch = async (input, init) => {
    const url = new URL(
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.href
          : input.url,
      window.location.origin
    );
    const bindingResponse = commandBindingResponse(url, init);
    if (bindingResponse) return bindingResponse;
    const payload =
      url.pathname === '/api/console/communications/types'
        ? [notificationType, commandBindingType]
        : url.pathname === '/api/console/communications/instances'
          ? [notificationInstance, commandBindingInstance]
          : url.pathname === '/api/console/communications/logs'
            ? {
                items: [notificationLog],
                total: 1,
                page: 1,
                page_size: 20
              }
            : url.pathname === '/api/console/communications/logs/log-1'
              ? notificationLog
              : undefined;
    if (payload !== undefined)
      return new Response(JSON.stringify({ data: payload, meta: null }), {
        headers: { 'content-type': 'application/json' }
      });
    return originalFetch(input, init);
  };
}

const messages: NotificationMessage[] = [
  {
    id: 'notification-unread',
    workspace_id: 'workspace-1',
    instance_id: 'notification-instance',
    recipient_user_id: 'user-1',
    title: '工作流运行完成，请检查处理结果',
    content: `第一行：团队工作流已完成。\n第二行：查看结果 https://example.com/${'notification-result-'.repeat(12)}`,
    read_at: null,
    created_at: '2026-10-13T00:00:00Z'
  },
  {
    id: 'notification-read',
    workspace_id: 'workspace-1',
    instance_id: 'notification-instance',
    recipient_user_id: 'user-1',
    title: '已处理的通知',
    content: '已读消息保留在当前收件箱中。',
    read_at: '2026-10-13T00:01:00Z',
    created_at: '2026-10-13T00:00:00Z'
  }
];

export function getStyleBoundaryNotificationResponse(
  url: URL,
  method: string
): Response | null {
  if (url.origin !== window.location.origin || method.toUpperCase() !== 'GET')
    return null;
  const payload =
    url.pathname === '/api/console/notifications/messages/unread-count'
      ? { unread_count: 1 }
      : url.pathname === '/api/console/notifications/channels'
        ? {
            items: [
              {
                instance_id: notificationInstance.id,
                display_name: notificationInstance.display_name,
                type_code: notificationInstance.type_code,
                deleted: false,
                latest_title: messages[0].title,
                latest_content: messages[0].content,
                latest_created_at: messages[0].created_at,
                unread_count: 1
              }
            ],
            next_cursor: null
          }
        : url.pathname === '/api/console/notifications/messages'
          ? { items: messages, next_cursor: null, unread_count: 1 }
          : undefined;
  if (payload !== undefined) {
    return new Response(JSON.stringify({ data: payload, meta: null }), {
      status: 200,
      headers: { 'content-type': 'application/json' }
    });
  }
  return null;
}

export function seedStyleBoundaryNotificationFetch() {
  const originalFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = new URL(
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.href
          : input.url,
      window.location.origin
    );
    const method =
      init?.method ?? (input instanceof Request ? input.method : 'GET');
    return (
      getStyleBoundaryNotificationResponse(url, method) ??
      originalFetch(input, init)
    );
  };
}

// The style scene supplies a stable connected signal, while the actual inbox
// consumes its HTTP snapshot and renders the production bell and drawer.
export class NotificationSceneEventSource extends EventTarget {
  onopen: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  private closed = false;
  constructor() {
    super();
    queueMicrotask(() => {
      if (!this.closed) this.onopen?.(new Event('open'));
    });
  }
  close() {
    this.closed = true;
  }
}

let installedEventSource: typeof EventSource | undefined;

export function seedStyleBoundaryNotificationEvents() {
  if (globalThis.EventSource === installedEventSource && installedEventSource)
    return;
  const originalEventSource = globalThis.EventSource;
  const fixtureEventSource = function (
    url: string | URL,
    init?: EventSourceInit
  ) {
    const target = new URL(String(url), window.location.origin);
    if (
      target.origin === window.location.origin &&
      target.pathname === '/api/console/notifications/events' &&
      !target.search
    ) {
      return new NotificationSceneEventSource();
    }
    return new originalEventSource(url, init);
  } as unknown as typeof EventSource;
  Object.assign(fixtureEventSource, {
    CONNECTING: originalEventSource.CONNECTING,
    OPEN: originalEventSource.OPEN,
    CLOSED: originalEventSource.CLOSED
  });
  fixtureEventSource.prototype = originalEventSource.prototype;
  installedEventSource = fixtureEventSource;
  globalThis.EventSource = fixtureEventSource;
}
