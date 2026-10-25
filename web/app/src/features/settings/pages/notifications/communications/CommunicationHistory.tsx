import { useMemo } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Empty, Flex, Select } from 'antd';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { useTranslation } from 'react-i18next';
import type { CommunicationConversationListItem } from '@1flowbase/api-client';
import { listCommunicationConversationOverview } from '../../../../notifications/api/communications';
import { notificationInstancesOptions } from '../../../../notifications/api/queries';
import { useAuthStore } from '../../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../../state/navigation-query-scope';
import {
  DataTable,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import { formatDateTime } from '../../../../../shared/i18n/format';
import {
  useCommunicationLocation,
  navigateCommunication
} from './communication-navigation';
import {
  CommunicationLink,
  CommunicationRunLink,
  ProcessingStatus,
  DeliveryStatus
} from './communication-presentation';
import { CommunicationMessagesTable } from './CommunicationMessagesTable';
import { CommunicationDetail } from './CommunicationDetail';

export function CommunicationHistory({
  records = false
}: {
  records?: boolean;
}) {
  const { t } = useTranslation('notifications');
  const scope = useAuthStore(selectNavigationQueryScope);
  const client = useQueryClient();
  const location = useCommunicationLocation();
  const instances = useQuery(notificationInstancesOptions(scope));
  const input = {
    instance_id: location.instance_id,
    page: location.page,
    page_size: 20
  };
  const conversations = useQuery({
    queryKey: ['communications', scope, 'management-conversations', input],
    enabled: !records,
    queryFn: ({ signal }) =>
      listCommunicationConversationOverview(input, signal)
  });
  const filters = {
    instance_id: location.instance_id,
    processing_status: location.processing_status,
    send_status: location.send_status
  };
  const resetSelection = {
    page: 1,
    conversation_id: undefined,
    message_id: undefined,
    conversation_page: 1
  };
  const toolbar = (
    <Flex gap="small" wrap>
      <Select
        allowClear
        className="notification-logs__filter"
        aria-label={t('display_name')}
        placeholder={t('all_channels')}
        value={location.instance_id}
        options={instances.data?.map((item) => ({
          value: item.id,
          label: item.display_name
        }))}
        onChange={(instance_id) =>
          navigateCommunication({ ...resetSelection, instance_id })
        }
      />
      {records && (
        <>
          <Select
            allowClear
            className="notification-logs__filter"
            aria-label={t('processing_status')}
            placeholder={t('processing_status')}
            value={location.processing_status}
            options={[
              { value: 'pending', label: t('processing_pending') },
              { value: 'processing', label: t('processing_active') },
              { value: 'succeeded', label: t('processing_succeeded') },
              { value: 'failed', label: t('processing_failed') }
            ]}
            onChange={(processing_status) =>
              navigateCommunication({ ...resetSelection, processing_status })
            }
          />
          <Select
            allowClear
            className="notification-logs__filter"
            aria-label={t('send_status')}
            placeholder={t('send_status')}
            value={location.send_status}
            options={[
              { value: 'none', label: t('delivery_none') },
              { value: 'pending', label: t('delivery_pending') },
              { value: 'sending', label: t('delivery_sending') },
              { value: 'succeeded', label: t('send_success') },
              { value: 'failed', label: t('send_failure') },
              { value: 'unknown', label: t('result_unconfirmed') }
            ]}
            onChange={(send_status) =>
              navigateCommunication({ ...resetSelection, send_status })
            }
          />
        </>
      )}
      <Button
        onClick={() =>
          navigateCommunication({
            ...resetSelection,
            instance_id: undefined,
            processing_status: undefined,
            send_status: undefined
          })
        }
      >
        {t('clear_filters')}
      </Button>
      <Button
        icon={<ReloadOutlined />}
        onClick={() =>
          void client.invalidateQueries({ queryKey: ['communications', scope] })
        }
      >
        {t('refresh')}
      </Button>
    </Flex>
  );
  const columns = useMemo<
    Array<DataTableColumn<CommunicationConversationListItem>>
  >(
    () => [
      {
        key: 'sender_display_name',
        title: t('conversation_user'),
        width: 150,
        mobileRole: 'title',
        render: (_, item) => (
          <CommunicationLink
            to={{
              conversation_id: item.id,
              message_id: undefined,
              conversation_page: 1
            }}
          >
            {item.sender_display_name ?? item.external_identity}
          </CommunicationLink>
        )
      },
      {
        key: 'instance_display_name',
        title: t('display_name'),
        width: 140,
        render: (_, item) => item.instance_display_name ?? t('deleted_channel')
      },
      {
        key: 'latest_message.received_at_ms',
        title: t('latest_message_at'),
        width: 170,
        render: (_, item) =>
          item.latest_message
            ? formatDateTime(
                new Date(item.latest_message.received_at_ms).toISOString()
              )
            : '—'
      },
      {
        key: 'latest_message.content_preview',
        title: t('latest_message'),
        width: 180,
        ellipsis: true,
        render: (_, item) =>
          item.latest_message ? (
            <CommunicationLink
              to={{
                conversation_id: item.id,
                message_id: item.latest_message.id,
                conversation_page: 1
              }}
            >
              {item.latest_message.content_preview || t('message_without_text')}
            </CommunicationLink>
          ) : (
            '—'
          )
      },
      {
        key: 'latest_message.processing_status',
        title: t('latest_processing_status'),
        width: 130,
        render: (_, item) =>
          item.latest_message ? (
            <ProcessingStatus status={item.latest_message.processing_status} />
          ) : (
            '—'
          )
      },
      {
        key: 'latest_message.send_status',
        title: t('latest_send_status'),
        width: 130,
        render: (_, item) =>
          item.latest_message ? (
            <DeliveryStatus status={item.latest_message.send_status} />
          ) : (
            '—'
          )
      },
      {
        key: 'latest_message.run_id',
        title: t('execution_logs'),
        width: 130,
        mobileRole: 'actions',
        render: (_, item) =>
          item.latest_message ? (
            <CommunicationRunLink item={item.latest_message} />
          ) : (
            '—'
          )
      }
    ],
    [t]
  );
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'communications.conversations.columns'
  });
  return (
    <section className="notifications-page">
      {(instances.isError || (!records && conversations.isError)) && (
        <Alert type="error" showIcon title={t('history_failed')} />
      )}
      {records ? (
        <CommunicationMessagesTable
          filters={filters}
          page={location.page}
          onPageChange={(page) => navigateCommunication({ page })}
          toolbar={toolbar}
        />
      ) : (
        <DataTable<CommunicationConversationListItem>
          className="notifications-page__table"
          columns={columns}
          configuration={configuration}
          rowKey="id"
          dataSource={conversations.data?.items ?? []}
          loading={conversations.isPending}
          page={conversations.data?.page ?? location.page}
          pageSize={20}
          total={conversations.data?.total ?? 0}
          onPageChange={(page) => navigateCommunication({ page })}
          toolbar={toolbar}
          emptyText={<Empty description={t('no_conversations')} />}
        />
      )}
      {(location.conversation_id || location.message_id) && (
        <CommunicationDetail />
      )}
    </section>
  );
}
