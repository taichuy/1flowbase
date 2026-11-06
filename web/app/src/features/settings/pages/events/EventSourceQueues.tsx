import type {
  EventRegisteredSourceSnapshot,
  EventSourceQueueSnapshot
} from '@1flowbase/api-client';
import { Alert, Button, Space, Typography } from 'antd';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { EventsTable } from './EventsTable';

export function EventSourceQueues({
  sources
}: {
  sources: EventRegisteredSourceSnapshot[];
}) {
  const { t } = useTranslation('settingsEvents');
  const counterLabels = {
    buffered: t('buffered'),
    in_flight: t('in_flight'),
    accepted: t('accepted'),
    dequeued: t('dequeued'),
    dropped: t('dropped')
  };
  const [registration, setRegistration] = useState<string>();
  const selected =
    sources.find((source) => source.registration_id === registration) ??
    sources[0];
  const columns: DataTableColumn<EventRegisteredSourceSnapshot>[] = [
    {
      key: 'name',
      title: t('name'),
      width: 220,
      ellipsis: true,
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          title={row.descriptor.name}
          styles={{
            root: { maxWidth: '100%' },
            content: {
              minWidth: 0,
              overflow: 'hidden',
              textOverflow: 'ellipsis'
            }
          }}
          onClick={() => setRegistration(row.registration_id)}
        >
          {row.descriptor.name}
        </Button>
      )
    },
    ...(['source_id', 'point_id', 'contract_id'] as const).map((key) => ({
      key,
      title: t(key),
      render: (_: unknown, row: EventRegisteredSourceSnapshot) =>
        row.descriptor[key],
      width: 220
    })),
    {
      key: 'registration_id',
      title: t('registration_id'),
      dataIndex: 'registration_id',
      width: 240
    },
    {
      key: 'queue_observation_complete',
      title: t('queue_observation'),
      width: 180,
      render: (_, row) => (
        <Typography.Text
          type={row.queue_observation_complete ? undefined : 'warning'}
        >
          {t(
            row.queue_observation_complete
              ? 'queue_observation_complete'
              : 'queue_observation_incomplete'
          )}
        </Typography.Text>
      )
    }
  ];
  const queueColumns: DataTableColumn<EventSourceQueueSnapshot>[] = [
    {
      key: 'queue_id',
      title: t('queue_id'),
      dataIndex: 'queue_id',
      width: 240,
      mobileRole: 'title'
    },
    {
      key: 'subscriber_id',
      title: t('original_subscriber_id'),
      dataIndex: 'subscriber_id',
      width: 240
    },
    {
      key: 'consumer_ids',
      title: t('consumer_ids'),
      width: 240,
      render: (_, row) => row.consumer_ids.join(', ') || '—'
    },
    ...(
      ['buffered', 'in_flight', 'accepted', 'dequeued', 'dropped'] as const
    ).map((key) => ({
      key,
      title: counterLabels[key],
      width: 130,
      render: (_: unknown, row: EventSourceQueueSnapshot) =>
        row[key] ?? t('unknown')
    })),
    {
      key: 'observed_at_unix_ms',
      title: t('observed_at'),
      width: 200,
      render: (_, row) => new Date(row.observed_at_unix_ms).toLocaleString()
    }
  ];
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Text strong>{t('original_sources')}</Typography.Text>
      <EventsTable
        tableKey="sources"
        rows={sources}
        columns={columns}
        rowKey="registration_id"
      />
      {selected && (
        <>
          <Typography.Text strong>
            {t('original_queues')} · {selected.descriptor.name}
          </Typography.Text>
          {!selected.queue_observation_complete && (
            <Alert
              type="warning"
              showIcon
              title={t('queue_observation_incomplete')}
              description={t('queue_observation_incomplete_help')}
            />
          )}
          <EventsTable
            key={selected.registration_id}
            tableKey="source-queues"
            rows={selected.queues}
            columns={queueColumns}
            rowKey="queue_id"
          />
        </>
      )}
    </Space>
  );
}
