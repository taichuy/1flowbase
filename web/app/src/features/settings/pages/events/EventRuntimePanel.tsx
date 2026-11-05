import {
  getEventRuntime,
  type EventRuntimeActivity,
  type EventRuntimeQueueSnapshot,
  type EventTriggerPhase,
  type EventTargetAdmissionStatus
} from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Descriptions, Space, Tag } from 'antd';
import { useTranslation } from 'react-i18next';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { EventsTable } from './EventsTable';
import { EventWorkflowSubscriptions } from './EventWorkflowSubscriptions';

export function EventRuntimePanel({ tab }: { tab: 'queues' | 'activity' }) {
  const { t } = useTranslation('settingsEvents');
  // This payload-free snapshot is local to the mounted observation page.
  const query = useQuery({
    queryKey: ['settings', 'events', 'runtime'],
    queryFn: () => getEventRuntime(),
    gcTime: 0
  });
  const phase = (value: EventTriggerPhase) =>
    ({
      subscribed: t('subscribed'),
      published: t('published'),
      consumed: t('consumed')
    })[value];
  const admission = (value: EventTargetAdmissionStatus | null) =>
    value === null
      ? '—'
      : {
          accepted: t('accepted'),
          no_consumer: t('no_consumer'),
          closed: t('closed'),
          resource_exhausted: t('resource_exhausted'),
          unconfigured: t('unconfigured'),
          invalid_body: t('invalid_body')
        }[value];
  const queueColumns: DataTableColumn<EventRuntimeQueueSnapshot>[] = [
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 220,
      mobileRole: 'title'
    },
    {
      key: 'contract_version',
      title: t('contract_version'),
      dataIndex: 'contract_version',
      width: 110
    },
    {
      key: 'subscription_id',
      title: t('subscription_id'),
      dataIndex: 'subscription_id',
      width: 220
    },
    {
      key: 'trigger_phase',
      title: t('phase'),
      width: 140,
      render: (_, row) => phase(row.trigger_phase)
    },
    {
      key: 'consumer_ids',
      title: t('consumer_ids'),
      width: 240,
      render: (_, row) => row.consumer_ids.join(', ') || t('no_consumer')
    },
    {
      key: 'closed',
      title: t('queue_state'),
      width: 120,
      render: (_, row) => (row.closed ? t('closed') : t('open'))
    },
    {
      key: 'buffered',
      title: t('buffered'),
      dataIndex: 'buffered',
      width: 110
    },
    {
      key: 'in_flight',
      title: t('in_flight'),
      dataIndex: 'in_flight',
      width: 130
    },
    {
      key: 'accepted',
      title: t('accepted'),
      dataIndex: 'accepted',
      width: 110
    },
    {
      key: 'consumed',
      title: t('dequeued'),
      dataIndex: 'consumed',
      width: 110
    },
    {
      key: 'admission_rejected',
      title: t('admission_rejected'),
      dataIndex: 'admission_rejected',
      width: 130
    }
  ];
  const activityColumns: DataTableColumn<EventRuntimeActivity>[] = [
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 220,
      mobileRole: 'title'
    },
    {
      key: 'contract_version',
      title: t('contract_version'),
      dataIndex: 'contract_version',
      width: 110
    },
    {
      key: 'phase',
      title: t('phase'),
      width: 140,
      render: (_, row) => phase(row.phase)
    },
    {
      key: 'admission',
      title: t('admission'),
      width: 180,
      render: (_, row) => (
        <Tag
          color={
            row.admission && row.admission !== 'accepted' ? 'error' : undefined
          }
        >
          {admission(row.admission)}
        </Tag>
      )
    },
    {
      key: 'observed_at_unix_ms',
      title: t('observed_at'),
      width: 200,
      render: (_, row) => new Date(row.observed_at_unix_ms).toLocaleString()
    },
    {
      key: 'event_id',
      title: t('event_id'),
      dataIndex: 'event_id',
      width: 240
    },
    {
      key: 'source_event_id',
      title: t('source_event_id'),
      dataIndex: 'source_event_id',
      width: 240,
      render: (_, row) => row.source_event_id ?? '—'
    },
    {
      key: 'subscription_id',
      title: t('subscription_id'),
      width: 240,
      render: (_, row) => row.subscription_id ?? '—'
    },
    {
      key: 'consumer_id',
      title: t('consumer_id'),
      width: 240,
      render: (_, row) => row.consumer_id ?? '—'
    }
  ];
  const snapshot = query.data;
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Alert
        type="info"
        showIcon
        title={t('ephemeral')}
        description={t('consumed_help')}
      />
      <Button onClick={() => void query.refetch()} loading={query.isFetching}>
        {t('refresh')}
      </Button>
      {query.isError && (
        <Alert type="error" showIcon title={t('runtime_error')} />
      )}
      {snapshot && (
        <>
          {snapshot.workflow_refresh_error !== null && (
            <Alert
              type="warning"
              showIcon
              title={t('workflow_refresh_error')}
            />
          )}
          {!snapshot.configured && (
            <Alert type="warning" showIcon title={t('unconfigured')} />
          )}
          <Descriptions
            size="small"
            column={{ xs: 1, sm: 2 }}
            items={[
              {
                key: 'instance_id',
                label: t('instance_id'),
                children: snapshot.instance_id
              },
              {
                key: 'workspace_id',
                label: t('workspace_id'),
                children: snapshot.workspace_id
              },
              {
                key: 'fetched_at',
                label: t('fetched_at'),
                children: new Date(query.dataUpdatedAt).toLocaleString()
              },
              {
                key: 'observation_ttl_ms',
                label: t('observation_window'),
                children: t('window_ms', { count: snapshot.observation_ttl_ms })
              },
              {
                key: 'admission_rejected',
                label: t('window_rejected'),
                children: snapshot.admission_rejected
              },
              {
                key: 'observation_bytes',
                label: t('observation_bytes'),
                children: `${snapshot.observation_bytes} / ${snapshot.observation_max_bytes}`
              },
              ...(snapshot.observation_loss.accounting_available
                ? [
                    {
                      key: 'window_started_at_unix_ms',
                      label: t('loss_window_start'),
                      children:
                        snapshot.observation_loss.window_started_at_unix_ms ===
                        null
                          ? '—'
                          : new Date(
                              snapshot.observation_loss
                                .window_started_at_unix_ms
                            ).toLocaleString()
                    },
                    {
                      key: 'window_duration_ms',
                      label: t('loss_window_duration'),
                      children: t('window_ms', {
                        count: snapshot.observation_loss.window_duration_ms
                      })
                    },
                    {
                      key: 'expired',
                      label: t('loss_expired'),
                      children: snapshot.observation_loss.expired
                    },
                    {
                      key: 'evicted',
                      label: t('loss_evicted'),
                      children: snapshot.observation_loss.evicted
                    },
                    {
                      key: 'oversize_dropped',
                      label: t('loss_oversize_dropped'),
                      children: snapshot.observation_loss.oversize_dropped
                    }
                  ]
                : [])
            ]}
          />
          <Alert
            type="info"
            title={
              snapshot.observation_loss.accounting_available
                ? t('loss_window_help')
                : t('loss_unavailable')
            }
          />
        </>
      )}
      {tab === 'queues' ? (
        <>
          {snapshot && (
            <EventWorkflowSubscriptions
              subscriptions={snapshot.workflow_subscriptions}
            />
          )}
          <Alert type="info" title={t('queue_counters_help')} />
          <EventsTable
            tableKey="queues"
            columns={queueColumns}
            rows={snapshot?.queues ?? []}
            rowKey="subscription_id"
            loading={query.isPending}
          />
        </>
      ) : (
        <EventsTable
          tableKey="activity"
          columns={activityColumns}
          rows={snapshot?.activity ?? []}
          rowKey={(row) =>
            JSON.stringify([
              row.event_id,
              row.phase,
              row.subscription_id,
              row.consumer_id,
              row.admission,
              row.observed_at_unix_ms
            ])
          }
          loading={query.isPending}
        />
      )}
    </Space>
  );
}
