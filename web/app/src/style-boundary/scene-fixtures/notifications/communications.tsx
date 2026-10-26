import type { CommunicationMessage } from '@1flowbase/api-client';
import { CommunicationChat } from '../../../features/settings/pages/notifications/communications/CommunicationChat';
import { getStyleBoundaryRequestUrl } from '../common-fetch';

const conversationId = 'style-chat-conversation';
const messages: CommunicationMessage[] = Array.from(
  { length: 5 },
  (_, index) => ({
    id: `style-chat-message-${index + 1}`,
    conversation_id: conversationId,
    input_id: `style-chat-input-${index + 1}`,
    instance_id: 'style-chat-channel',
    instance_display_name: '团队沟通渠道',
    external_identity: 'alice',
    sender_display_name: 'Alice',
    type: index === 4 ? 'file' : 'text',
    direction: index % 2 === 0 ? 'outbound' : 'inbound',
    occurred_at_ms: 1792960000000 + index * 1000,
    content:
      index === 4
        ? null
        : [
            '已收到你的请求。',
            '请整理本周工作流运行情况。https://example.com/' +
              'long-reference'.repeat(12),
            '已汇总完成，报告包含运行结果与待处理项。',
            '请发送报告文件。'
          ][index]!,
    file:
      index === 4
        ? {
            file_name: '工作流运行汇总报告-2026-10.pdf',
            media_type: 'application/pdf',
            size_bytes: 2048,
            status: 'ready',
            file_table_id: 'files',
            file_record_id: 'report',
            failure_code: null,
            access_path:
              '/api/console/communications/instances/style-chat-channel/files/style-chat-message-5'
          }
        : null,
    run_id: 'style-chat-run',
    application_id: 'style-chat-application',
    processing_status: 'succeeded',
    send_status: index === 4 ? 'failed' : 'succeeded',
    failure_code: index === 4 ? 'send_interrupted' : null,
    can_retry: index === 4
  })
);

export function seedStyleBoundaryCommunicationChatFetch() {
  const originalFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = new URL(
      getStyleBoundaryRequestUrl(input),
      window.location.origin
    );
    let data: unknown;
    if (url.pathname === '/api/console/communications/conversations') {
      data = {
        items: [
          {
            id: conversationId,
            workspace_id: 'workspace-1',
            instance_id: 'style-chat-channel',
            instance_display_name: '团队沟通渠道',
            external_identity: 'alice',
            external_conversation_key: 'style-chat-thread',
            sender_display_name: 'Alice'
          }
        ],
        total: 1,
        page: 1,
        page_size: 1
      };
    } else if (
      url.pathname ===
      `/api/console/communications/conversations/${conversationId}/messages`
    ) {
      data = { items: messages, next_cursor: null };
    } else {
      return originalFetch(input, init);
    }
    return new Response(JSON.stringify({ data, meta: null }), {
      headers: { 'content-type': 'application/json' }
    });
  };
}

export function CommunicationChatStyleBoundaryScene() {
  return <CommunicationChat conversation_id={conversationId} />;
}
