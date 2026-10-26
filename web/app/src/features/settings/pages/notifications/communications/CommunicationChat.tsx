import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient
} from '@tanstack/react-query';
import { Alert, Button, Space, Spin, Tooltip, Typography } from 'antd';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import RedoOutlined from '@ant-design/icons/es/icons/RedoOutlined';
import { useTranslation } from 'react-i18next';
import type { CommunicationMessage } from '@1flowbase/api-client';
import {
  listCommunicationChatMessages,
  listCommunicationConversationOverview,
  retryCommunicationMessage
} from '../../../../notifications/api/communications';
import { DebugConversationPane } from '../../../../agent-flow/components/debug-console/conversation/DebugConversationPane';
import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../../../agent-flow/api/runtime';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { navigateCommunication } from './communication-navigation';
import {
  CommunicationFile,
  CommunicationRunLink,
  MessageStatus
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
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const client = useQueryClient();
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
  const retry = useMutation({
    mutationFn: (message: CommunicationMessage) =>
      retryCommunicationMessage(message.instance_id, message.id, csrfToken),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ['communications', scope] })
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
    return (
      <>
        <Typography.Text
          type="secondary"
          className="communication-chat__timestamp"
        >
          {formatDateTime(new Date(item.occurred_at_ms).toISOString())}
        </Typography.Text>
        <CommunicationRunLink item={item} />
        <Tooltip title={<MessageStatus message={item} />}>
          <Typography.Text type="secondary">
            {item.direction === 'inbound'
              ? t('chat_received')
              : t('chat_reply')}
          </Typography.Text>
        </Tooltip>
        {item.can_retry && (
          <Tooltip title={t('retry_delivery')}>
            <Button
              size="small"
              type="text"
              icon={<RedoOutlined />}
              aria-label={t('retry_delivery')}
              loading={retry.isPending && retry.variables?.id === item.id}
              disabled={retry.isPending}
              onClick={() => retry.mutate(item)}
            />
          </Tooltip>
        )}
      </>
    );
  };
  const info = conversation.data?.items[0];
  return (
    <ResizableDrawer
      open
      title={t('chat')}
      resizeLabel={t('resize_drawer')}
      onClose={() => navigateCommunication({ conversation_id: undefined })}
      bodyClassName="communication-chat"
      extra={
        <Button
          icon={<ReloadOutlined aria-hidden="true" />}
          loading={history.isFetching}
          onClick={() => void history.refetch()}
        >
          {t('refresh')}
        </Button>
      }
    >
      {info && (
        <Space wrap className="communication-chat__identity">
          <Typography.Text>
            {info.instance_display_name ?? t('deleted_channel')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {info.sender_display_name ?? info.external_identity}
          </Typography.Text>
        </Space>
      )}
      {(history.isError || conversation.isError || retry.isError) && (
        <Alert type="error" showIcon title={t('history_failed')} />
      )}
      {history.isPending && <Spin />}
      {history.hasNextPage && (
        <Button
          className="communication-chat__earlier"
          loading={history.isFetchingNextPage}
          onClick={() => void history.fetchNextPage()}
        >
          {t('load_earlier_messages')}
        </Button>
      )}
      <DebugConversationPane
        messages={messages}
        runContext={readOnlyContext}
        status="idle"
        stopping={false}
        showComposer={false}
        userMessageMainRender={renderFile}
        assistantMessageMainRender={renderFile}
        userMessageActions={renderActions}
        assistantMessageActions={renderActions}
        onChangeQuery={noAction}
        onStopRun={noAction}
        onSubmitPrompt={noAction}
      />
    </ResizableDrawer>
  );
}
