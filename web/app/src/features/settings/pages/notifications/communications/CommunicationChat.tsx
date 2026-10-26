import { ReadOnlyConversationInput } from '../../../../agent-flow/components/debug-console/conversation/read-only/ReadOnlyConversationInput';
import { createPortal } from 'react-dom';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { Alert, Button, Space, Spin, Typography } from 'antd';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { useTranslation } from 'react-i18next';
import {
  communicationBindingOptions,
  listCommunicationChatMessages,
  listCommunicationConversationOverview
} from '../../../../notifications/api/communications';
import { AgentFlowDebugConsole } from '../../../../agent-flow/components/debug-console/AgentFlowDebugConsole';
import { ApplicationLogsFloatingWindow } from '../../../../applications/components/logs/ApplicationLogsFloatingWindow';
import '../../../../applications/pages/application-logs-page.css';
import '../../../../applications/components/logs/application-run-detail-panel.css';
import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../../../agent-flow/api/runtime';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { navigateCommunication } from './communication-navigation';
import {
  CommunicationFile,
  CommunicationRunLink
} from './communication-presentation';
import './communication-chat.css';

const readOnlyContext: AgentFlowRunContext = {
  environmentLabel: 'published',
  remembered: false,
  fields: []
};
const noAction = () => {};

export function CommunicationChat({
  conversation_id
}: {
  conversation_id: string;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const conversation = useQuery({
    queryKey: [
      'communications',
      scope,
      'management-conversation',
      conversation_id
    ],
    queryFn: ({ signal }) =>
      listCommunicationConversationOverview(
        { conversation_id, page: 1, page_size: 1 },
        signal
      )
  });
  const binding = useQuery(
    communicationBindingOptions(scope, conversation.data?.items[0]?.instance_id)
  );
  const history = useInfiniteQuery({
    queryKey: ['communications', scope, 'chat', conversation_id],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam, signal }) =>
      listCommunicationChatMessages(
        conversation_id,
        pageParam ? { limit: 5, cursor: pageParam } : { limit: 5 },
        signal
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    // A newly opened chat starts with five messages, even after an earlier visit loaded more.
    gcTime: 0,
    refetchOnWindowFocus: false
  });
  // Each server page is chronological; pages arrive newest first, older pages prepend.
  const items =
    history.data?.pages
      .slice()
      .reverse()
      .flatMap((page) => page.items) ?? [];
  const byId = new Map(items.map((item) => [item.id, item]));
  const messages: AgentFlowDebugMessage[] = items.map((item) => ({
    id: item.id,
    role: item.direction === 'inbound' ? 'user' : 'assistant',
    content: item.content ?? '',
    status: 'completed',
    runId: item.run_id,
    canOpenDetail: false,
    rawOutput: null,
    traceSummary: []
  }));
  const renderFile = (message: AgentFlowDebugMessage) => {
    const item = byId.get(message.id)!;
    return item.type === 'file' ? (
      <CommunicationFile message={item} />
    ) : undefined;
  };
  const renderActions = (message: AgentFlowDebugMessage) => {
    const item = byId.get(message.id)!;
    return <CommunicationRunLink item={item} />;
  };
  const info = conversation.data?.items[0];
  return createPortal(
    <ApplicationLogsFloatingWindow
      active
      initialRect={() => {
        const width = Math.min(720, window.innerWidth - 32);
        const height = Math.min(840, window.innerHeight - 96);
        return {
          left: Math.max(16, window.innerWidth - width - 24),
          top: 72,
          width,
          height
        };
      }}
      testId="communication-chat-window"
      title={t('chat')}
      onActivate={noAction}
    >
      <div className="application-run-detail__conversation-pane communication-chat">
        {(history.isError || conversation.isError) && (
          <Alert type="error" showIcon title={t('history_failed')} />
        )}
        {history.isPending && <Spin />}
        <AgentFlowDebugConsole
          title={t('chat')}
          closeLabel={t('chat_close')}
          onClose={() => navigateCommunication({ conversation_id: undefined })}
          subtitle={
            info ? (
              <Space wrap className="communication-chat__identity">
                <span>
                  {info.instance_display_name ?? t('deleted_channel')}
                </span>
                <span>
                  {info.sender_display_name ?? info.external_identity}
                </span>
              </Space>
            ) : undefined
          }
          headerActions={
            <>
              {history.hasNextPage && (
                <Button
                  size="small"
                  className="communication-chat__earlier"
                  loading={history.isFetchingNextPage}
                  onClick={() => void history.fetchNextPage()}
                >
                  {t('load_earlier_messages')}
                </Button>
              )}
              <Button
                size="small"
                type="text"
                aria-label={t('refresh')}
                icon={<ReloadOutlined aria-hidden="true" />}
                loading={history.isFetching}
                onClick={() => void history.refetch()}
              />
            </>
          }
          messages={messages}
          runContext={readOnlyContext}
          status="idle"
          stopping={false}
          showComposer={false}
          showClearAction={false}
          messageFooterStart={(message) => (
            <Typography.Text
              type="secondary"
              className="communication-chat__timestamp"
            >
              {formatDateTime(
                new Date(byId.get(message.id)!.occurred_at_ms).toISOString()
              )}
            </Typography.Text>
          )}
          userMessageMainRender={renderFile}
          assistantMessageMainRender={renderFile}
          userMessageActions={renderActions}
          assistantMessageActions={renderActions}
          onChangeRunContextValue={noAction}
          onClearSession={noAction}
          onStopRun={noAction}
          onSubmitPrompt={noAction}
        />
        <ReadOnlyConversationInput
          model={binding.data?.binding?.model}
          reasoning_effort={binding.data?.binding?.reasoning_effort}
        />
      </div>
    </ApplicationLogsFloatingWindow>,
    document.body
  );
}
