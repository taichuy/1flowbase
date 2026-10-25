import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Descriptions, Space, Spin, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  getCommunicationMessageDetail,
  listCommunicationConversationOverview,
  retryCommunicationMessage
} from '../../../../notifications/api/communications';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { formatDateTime } from '../../../../../shared/i18n/format';
import {
  useCommunicationLocation,
  navigateCommunication
} from './communication-navigation';
import {
  CommunicationFile,
  CommunicationLink,
  CommunicationRunLink,
  MessageStatus
} from './communication-presentation';
import { CommunicationMessagesTable } from './CommunicationMessagesTable';

export function CommunicationDetail() {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const client = useQueryClient();
  const location = useCommunicationLocation();
  const { message_id, conversation_id } = location;
  const message = useQuery({
    queryKey: ['communications', scope, 'management-message', message_id],
    enabled: !!message_id,
    queryFn: ({ signal }) => getCommunicationMessageDetail(message_id!, signal)
  });
  const conversation = useQuery({
    queryKey: [
      'communications',
      scope,
      'management-conversation',
      conversation_id
    ],
    enabled: !!conversation_id && !message_id,
    queryFn: ({ signal }) =>
      listCommunicationConversationOverview(
        { conversation_id, page: 1, page_size: 1 },
        signal
      )
  });
  const retry = useMutation({
    mutationFn: () =>
      retryCommunicationMessage(
        message.data!.instance_id,
        message.data!.id,
        csrfToken
      ),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ['communications', scope] })
  });
  const item = message_id ? message.data : conversation.data?.items[0];
  const query = message_id ? message : conversation;
  const detail = message_id ? message.data : undefined;
  return (
    <ResizableDrawer
      open
      title={
        message_id ? t('received_record_detail') : t('conversation_detail')
      }
      onClose={() =>
        navigateCommunication({
          message_id: undefined,
          conversation_id: undefined,
          conversation_page: 1
        })
      }
      resizeLabel={t('resize_drawer')}
      rootClassName="notification-log-detail"
    >
      <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
        <Space wrap>
          {message_id && conversation_id && (
            <CommunicationLink to={{ message_id: undefined }}>
              {t('back_to_conversation')}
            </CommunicationLink>
          )}
          <Button
            loading={query.isFetching}
            onClick={() =>
              void client.invalidateQueries({
                queryKey: ['communications', scope]
              })
            }
          >
            {t('refresh')}
          </Button>
        </Space>
        {(query.isError || retry.isError) && (
          <Alert type="error" showIcon title={t('history_failed')} />
        )}
        {query.isPending && <Spin />}
        {!message_id && conversation.isSuccess && !item && (
          <Alert type="info" title={t('conversation_unavailable')} />
        )}
        {item && (
          <>
            <section className="notification-log-detail__form">
              <Descriptions
                column={1}
                items={[
                  {
                    key: 'id',
                    label: message_id ? t('message_id') : t('conversation_id'),
                    children: (
                      <Typography.Text copyable>{item.id}</Typography.Text>
                    )
                  },
                  {
                    key: 'channel',
                    label: t('display_name'),
                    children: item.instance_display_name ?? t('deleted_channel')
                  },
                  {
                    key: 'sender',
                    label: t('conversation_user'),
                    children: item.sender_display_name ?? item.external_identity
                  },
                  {
                    key: 'external_identity',
                    label: t('user_identifier'),
                    children: item.external_identity
                  }
                ]}
              />
              {detail ? (
                <>
                  <Descriptions
                    column={1}
                    items={[
                      {
                        key: 'received_at',
                        label: t('received_at'),
                        children: formatDateTime(
                          new Date(detail.received_at_ms).toISOString()
                        )
                      },
                      {
                        key: 'conversation',
                        label: t('conversation_detail'),
                        children: (
                          <CommunicationLink
                            to={{
                              tab: 'conversations',
                              conversation_id: detail.conversation_id,
                              message_id: undefined,
                              conversation_page: 1
                            }}
                          >
                            {t('view_conversation')}
                          </CommunicationLink>
                        )
                      },
                      {
                        key: 'status',
                        label: t('status'),
                        children: <MessageStatus message={detail} />
                      },
                      {
                        key: 'run',
                        label: t('execution_logs'),
                        children: <CommunicationRunLink item={detail} />
                      }
                    ]}
                  />
                  <Typography.Title level={5}>
                    {t('received_message')}
                  </Typography.Title>
                  <Typography.Paragraph className="notification-log-detail__text">
                    {detail.content ?? t('message_without_text')}
                  </Typography.Paragraph>
                  <Space orientation="vertical">
                    {detail.attachments.map((file) => (
                      <CommunicationFile
                        key={file.id}
                        file={file}
                        instance_id={detail.instance_id}
                      />
                    ))}
                  </Space>
                  {detail.final_reply !== null && (
                    <>
                      <Typography.Title level={5}>
                        {t('assistant_reply')}
                      </Typography.Title>
                      <Typography.Paragraph className="notification-log-detail__text">
                        {detail.final_reply}
                      </Typography.Paragraph>
                    </>
                  )}
                  {detail.can_retry && (
                    <Button
                      loading={retry.isPending}
                      onClick={() => retry.mutate()}
                    >
                      {t('retry_delivery')}
                    </Button>
                  )}
                </>
              ) : null}
            </section>
            {!detail && (
              <CommunicationMessagesTable
                conversation_id={conversation_id}
                page={location.conversation_page}
                onPageChange={(conversation_page) =>
                  navigateCommunication({ conversation_page })
                }
              />
            )}
          </>
        )}
      </Space>
    </ResizableDrawer>
  );
}
