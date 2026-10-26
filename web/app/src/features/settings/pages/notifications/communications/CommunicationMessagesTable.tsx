import { useMemo } from 'react';
import { Alert, Button, Empty } from 'antd';
import MessageOutlined from '@ant-design/icons/es/icons/MessageOutlined';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type {
  CommunicationMessageListInput,
  CommunicationMessageListItem
} from '@1flowbase/api-client';
import { listCommunicationMessageOverview } from '../../../../notifications/api/communications';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import {
  DataTable,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { ProcessingStatus, DeliveryStatus } from './communication-presentation';
import { navigateCommunication } from './communication-navigation';
import type { ReactNode } from 'react';

export function CommunicationMessagesTable({
  filters,
  page,
  onPageChange,
  toolbar,
  conversation_id
}: {
  filters?: Omit<CommunicationMessageListInput, 'page' | 'page_size'>;
  page: number;
  onPageChange: (page: number) => void;
  toolbar?: ReactNode;
  conversation_id?: string;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const input = {
    direction: 'inbound' as const,
    ...filters,
    ...(conversation_id ? { conversation_id } : {}),
    page,
    page_size: 20
  };
  const messages = useQuery({
    queryKey: ['communications', scope, 'management-messages', input],
    queryFn: ({ signal }) => listCommunicationMessageOverview(input, signal)
  });
  const columns = useMemo<Array<DataTableColumn<CommunicationMessageListItem>>>(
    () => [
      {
        key: 'occurred_at_ms',
        title: t('received_at'),
        width: 170,
        render: (_, item) =>
          formatDateTime(new Date(item.occurred_at_ms).toISOString())
      },
      {
        key: 'content_preview',
        title: t('received_message'),
        width: 220,
        ellipsis: true,
        mobileRole: 'title',
        render: (_, item) =>
          item.type === 'file' ? item.file?.file_name : item.content_preview
      },
      {
        key: 'sender_display_name',
        title: t('conversation_user'),
        width: 170,
        render: (_, item) => item.sender_display_name ?? item.external_identity
      },
      {
        key: 'instance_display_name',
        title: t('display_name'),
        width: 140,
        render: (_, item) => item.instance_display_name ?? t('deleted_channel')
      },
      {
        key: 'processing_status',
        title: t('processing_status'),
        width: 130,
        render: (_, item) => (
          <ProcessingStatus status={item.processing_status} />
        )
      },
      {
        key: 'send_status',
        title: t('send_status'),
        width: 130,
        render: (_, item) => <DeliveryStatus status={item.send_status} />
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 110,
        mobileRole: 'actions',
        render: (_, item) => (
          <Button
            type="link"
            icon={<MessageOutlined aria-hidden="true" />}
            onClick={() =>
              navigateCommunication({ conversation_id: item.conversation_id })
            }
          >
            {t('chat')}
          </Button>
        )
      }
    ],
    [t]
  );
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'communications.received.timeline.columns'
  });
  return (
    <>
      {messages.isError && (
        <Alert type="error" showIcon title={t('history_failed')} />
      )}
      <DataTable<CommunicationMessageListItem>
        className="notifications-page__table"
        columns={columns}
        configuration={configuration}
        rowKey="id"
        dataSource={messages.data?.items ?? []}
        loading={messages.isPending}
        page={messages.data?.page ?? page}
        pageSize={20}
        total={messages.data?.total ?? 0}
        onPageChange={onPageChange}
        toolbar={toolbar}
        emptyText={<Empty description={t('empty')} />}
      />
    </>
  );
}
