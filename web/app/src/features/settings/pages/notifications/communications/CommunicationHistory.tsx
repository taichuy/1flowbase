import { useState } from 'react';
import { useInfiniteQuery, useMutation, useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Empty,
  Flex,
  List,
  Select,
  Space,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';
import type {
  CommunicationAttachment,
  CommunicationMessage
} from '@1flowbase/api-client';
import {
  listCommunicationConversations,
  listCommunicationMessages,
  listCommunicationFiles,
  retryCommunicationMessage,
  getCommunicationFile
} from '../../../../notifications/api/communications';
import { notificationInstancesOptions } from '../../../../notifications/api/queries';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { formatDateTime } from '../../../../../shared/i18n/format';

export function CommunicationHistory({
  records = false
}: {
  records?: boolean;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const instances = useQuery(notificationInstancesOptions(scope));
  const [instance_id, setInstanceId] = useState<string>();
  const [conversation_id, setConversationId] = useState<string>();
  const conversations = useInfiniteQuery({
    queryKey: ['communications', scope, instance_id, 'conversations'],
    enabled: !!instance_id,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam, signal }) =>
      listCommunicationConversations(
        instance_id!,
        { ...(pageParam ? { cursor: pageParam } : {}), limit: 20 },
        signal
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined
  });
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Flex gap="small" wrap>
        <Select
          style={{ minWidth: 220 }}
          aria-label={t('display_name')}
          placeholder={t('select_communication_channel')}
          value={instance_id}
          options={instances.data?.map((i) => ({
            value: i.id,
            label: i.display_name
          }))}
          onChange={(id) => {
            setInstanceId(id);
            setConversationId(undefined);
          }}
        />
        <Button
          loading={conversations.isFetching}
          disabled={!instance_id}
          onClick={() => void conversations.refetch()}
        >
          {t('refresh')}
        </Button>
      </Flex>
      {(instances.isError || conversations.isError) && (
        <Alert type="error" title={t('history_failed')} />
      )}
      {!instance_id ? (
        <Empty description={t('select_communication_channel')} />
      ) : (
        <>
          <List
            loading={conversations.isPending}
            dataSource={conversations.data?.pages.flatMap((p) => p.items) ?? []}
            locale={{ emptyText: t('no_conversations') }}
            renderItem={(item) => (
              <List.Item
                actions={[
                  <Button
                    key="detail"
                    type="link"
                    onClick={() => setConversationId(item.id)}
                  >
                    {t('detail')}
                  </Button>
                ]}
              >
                <List.Item.Meta
                  title={item.sender_display_name ?? item.external_identity}
                  description={item.external_conversation_key}
                />
              </List.Item>
            )}
          />
          {conversations.hasNextPage && (
            <Button
              loading={conversations.isFetchingNextPage}
              onClick={() => void conversations.fetchNextPage()}
            >
              {t('load_more')}
            </Button>
          )}
        </>
      )}
      {instance_id && conversation_id && (
        <CommunicationConversationDetail
          key={`${instance_id}:${conversation_id}`}
          instance_id={instance_id}
          conversation_id={conversation_id}
          records={records}
          onClose={() => setConversationId(undefined)}
        />
      )}
    </Space>
  );
}

function CommunicationConversationDetail({
  instance_id,
  conversation_id,
  records,
  onClose
}: {
  instance_id: string;
  conversation_id: string;
  records: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const csrfToken = useAuthStore((s) => s.csrfToken) ?? '';
  const messages = useInfiniteQuery({
    queryKey: [
      'communications',
      scope,
      instance_id,
      conversation_id,
      'messages'
    ],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam, signal }) =>
      listCommunicationMessages(
        instance_id,
        conversation_id,
        { ...(pageParam ? { cursor: pageParam } : {}), limit: 20 },
        signal
      ),
    getNextPageParam: (p) => p.next_cursor ?? undefined
  });
  const files = useInfiniteQuery({
    queryKey: ['communications', scope, instance_id, conversation_id, 'files'],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam, signal }) =>
      listCommunicationFiles(
        instance_id,
        conversation_id,
        { ...(pageParam ? { cursor: pageParam } : {}), limit: 20 },
        signal
      ),
    getNextPageParam: (p) => p.next_cursor ?? undefined
  });
  const retry = useMutation({
    mutationFn: (message_id: string) =>
      retryCommunicationMessage(instance_id, message_id, csrfToken),
    onSuccess: () => messages.refetch()
  });
  const attachments = files.data?.pages.flatMap((p) => p.items) ?? [];
  return (
    <ResizableDrawer
      open
      title={records ? t('send_receive_records') : t('conversation_detail')}
      onClose={onClose}
      resizeLabel={t('resize_drawer')}
    >
      <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
        <Button
          loading={messages.isFetching || files.isFetching}
          onClick={() => {
            void messages.refetch();
            void files.refetch();
          }}
        >
          {t('refresh')}
        </Button>
        {(messages.isError || files.isError || retry.isError) && (
          <Alert type="error" title={t('history_failed')} />
        )}
        <List
          loading={messages.isPending}
          dataSource={messages.data?.pages.flatMap((p) => p.items) ?? []}
          locale={{ emptyText: t('empty') }}
          renderItem={(item) => (
            <List.Item>
              <Space orientation="vertical" style={{ width: '100%' }}>
                <Typography.Text type="secondary">
                  {formatDateTime(new Date(item.received_at_ms).toISOString())}
                </Typography.Text>
                <Typography.Text strong>
                  {t('received_message')}
                </Typography.Text>
                <Typography.Paragraph style={{ whiteSpace: 'pre-wrap' }}>
                  {item.content ?? '—'}
                </Typography.Paragraph>
                <MessageStatus message={item} />
                {attachments
                  .filter((file) => file.message_id === item.id)
                  .map((file) => (
                    <CommunicationFile
                      key={file.id}
                      file={file}
                      instance_id={instance_id}
                    />
                  ))}
                {item.final_reply !== null && (
                  <>
                    <Typography.Text strong>
                      {t('assistant_reply')}
                    </Typography.Text>
                    <Typography.Paragraph style={{ whiteSpace: 'pre-wrap' }}>
                      {item.final_reply}
                    </Typography.Paragraph>
                  </>
                )}
                {item.can_retry && (
                  <Button
                    loading={retry.isPending && retry.variables === item.id}
                    disabled={retry.isPending}
                    onClick={() => retry.mutate(item.id)}
                  >
                    {t('retry_delivery')}
                  </Button>
                )}
              </Space>
            </List.Item>
          )}
        />
        {messages.hasNextPage && (
          <Button
            loading={messages.isFetchingNextPage}
            onClick={() => void messages.fetchNextPage()}
          >
            {t('load_more')}
          </Button>
        )}
        {files.hasNextPage && (
          <Button
            loading={files.isFetchingNextPage}
            onClick={() => void files.fetchNextPage()}
          >
            {t('load_more_files')}
          </Button>
        )}
      </Space>
    </ResizableDrawer>
  );
}

export function MessageStatus({ message }: { message: CommunicationMessage }) {
  const { t } = useTranslation('notifications');
  const processing = {
    pending: t('processing_pending'),
    processing: t('processing_active'),
    succeeded: t('processing_succeeded'),
    failed: t('processing_failed')
  };
  const delivery = {
    none: t('delivery_none'),
    pending: t('delivery_pending'),
    sending: t('delivery_sending'),
    succeeded: t('send_success'),
    failed: t('send_failure'),
    unknown: t('result_unconfirmed')
  };
  return (
    <Space wrap>
      <Tag color={message.processing_status === 'failed' ? 'error' : undefined}>
        {processing[message.processing_status]}
      </Tag>
      <Tag color={message.send_status === 'failed' ? 'error' : undefined}>
        {delivery[message.send_status]}
      </Tag>
      {message.failure_code && (
        <Typography.Text type="danger">
          {t('failure_reason')}: {failureLabel(message.failure_code, t)}
        </Typography.Text>
      )}
    </Space>
  );
}
function CommunicationFile({
  file,
  instance_id
}: {
  file: CommunicationAttachment;
  instance_id: string;
}) {
  const { t } = useTranslation('notifications');
  const link = useMutation({
    mutationFn: () => getCommunicationFile(instance_id, file.id)
  });
  return (
    <Space wrap>
      <Typography.Text>{file.file_name}</Typography.Text>
      <Tag>
        {file.status === 'ready'
          ? t('file_ready')
          : file.status === 'failed'
            ? t('file_failed')
            : t('file_pending')}
      </Tag>
      {file.status === 'ready' && file.file_table_id && file.file_record_id && (
        <Button loading={link.isPending} onClick={() => link.mutate()}>
          {t('file_access')}
        </Button>
      )}
      {link.data?.url && (
        <Typography.Link
          href={link.data.url}
          target="_blank"
          rel="noopener noreferrer"
        >
          {t('download_file')}
        </Typography.Link>
      )}
      {(file.failure_code || link.isError || (link.data && !link.data.url)) && (
        <Typography.Text type="danger">
          {file.failure_code
            ? failureLabel(file.failure_code, t)
            : t('file_access_failed')}
        </Typography.Text>
      )}
    </Space>
  );
}

function failureLabel(code: string, t: (key: string) => string) {
  switch (code) {
    case 'run_interrupted':
      return t('failure_run_interrupted');
    case 'send_interrupted':
      return t('failure_send_interrupted');
    case 'download_failed':
      return t('failure_download_failed');
    case 'upload_failed':
      return t('failure_upload_failed');
    case 'run_failed':
      return t('failure_run_failed');
    case 'no_final_answer':
      return t('failure_no_final_answer');
    case 'provider_rejected':
      return t('failure_provider_rejected');
    default:
      return t('failure_reason_unavailable');
  }
}
