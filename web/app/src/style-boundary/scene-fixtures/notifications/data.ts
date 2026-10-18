import type {
  NotificationInstance,
  NotificationMessage,
  NotificationType
} from '@1flowbase/api-client';

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

export function seedStyleBoundaryNotificationSettingsFetch() {
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
    const payload =
      url.pathname === '/api/console/notifications/types'
        ? [notificationType]
        : url.pathname === '/api/console/notifications/instances'
          ? [notificationInstance]
          : url.pathname === '/api/console/notifications/logs'
            ? {
                items: [
                  {
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
                    content: '工作流已成功运行。',
                    recipient_count: 2,
                    created_at: '2026-10-13T00:00:00Z',
                    completed_at: '2026-10-13T00:01:00Z'
                  }
                ],
                total: 1,
                page: 1,
                page_size: 20
              }
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
    return originalFetch(input, init);
  };
}
