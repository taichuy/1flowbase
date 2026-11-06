import {
  getEventRuntime,
  type EventRuntimeActivity,
  type EventSourceScope,
  type EventRuntimeQueueSnapshot,
  type EventTriggerPhase,
  type EventTargetAdmissionStatus
} from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Select,
  Space,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { EventsTable } from './EventsTable';
import { EventSourceQueues } from './EventSourceQueues';
import { navigateEventScope, useEventScope } from './event-navigation';
import { EventWorkflowSubscriptions } from './EventWorkflowSubscriptions';

export function EventRuntimePanel({ tab }: { tab: 'queues' | 'activity' }) {
  const { t } = useTranslation('settingsEvents');
  const requestedScope = useEventScope();
  const scopeKey = (scope: EventSourceScope) =>
    scope.kind === 'workspace' ? `workspace:${scope.workspace_id}` : scope.kind;
  const scopeLabel = (scope: EventSourceScope) =>
    scope.kind === 'workspace'
      ? `${t('workspace_scope')} · ${scope.workspace_id}`
      : { system: t('system'), unbound: t('unbound') }[scope.kind];
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
          reentrant: t('reentrant'),
          no_consumer: t('no_consumer'),
          closed: t('closed'),
          resource_exhausted: t('resource_exhausted'),
          unconfigured: t('unconfigured'),
          invalid_body: t('invalid_body')
        }[value];
  const queueColumns: DataTableColumn<EventRuntimeQueueSnapshot>[] = [
    {
      key: 'source_target',
      title: t('source_target'),
      width: 300,
      render: (_, row) =>
        row.source_target
          ? `${scopeLabel(row.source_target.scope)} · ${row.source_target.source_id} · ${row.source_target.workflow_id}`
          : '—'
    },
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 220,
      mobileRole: 'title'
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
  const nativeLabels = {
    source_id: t('source_id'),
    point_id: t('point_id'),
    original_event_id: t('original_event_id'),
    producer_id: t('producer_id'),
    registration_id: t('registration_id')
  };
  const originalLabels = {
    original_subscriber_id: t('original_subscriber_id'),
    original_consumer_id: t('original_consumer_id')
  };
  const activityColumns: DataTableColumn<EventRuntimeActivity>[] = [
    ...(
      [
        'source_id',
        'point_id',
        'original_event_id',
        'producer_id',
        'registration_id'
      ] as const
    ).map((key) => ({
      key,
      title: nativeLabels[key],
      width: 240,
      render: (_: unknown, row: EventRuntimeActivity) =>
        row.source_context?.[key] ?? '—'
    })),
    {
      key: 'scope',
      title: t('scope'),
      width: 220,
      render: (_, row) =>
        row.source_context ? scopeLabel(row.source_context.scope) : '—'
    },
    ...(['original_subscriber_id', 'original_consumer_id'] as const).map(
      (key) => ({
        key,
        title: originalLabels[key],
        width: 240,
        render: (_: unknown, row: EventRuntimeActivity) => row[key] ?? '—'
      })
    ),
    {
      key: 'contract_id',
      title: t('contract_id'),
      dataIndex: 'contract_id',
      width: 220,
      mobileRole: 'title'
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
  const selectedView =
    snapshot?.source_views.find(
      (view) => scopeKey(view.scope) === requestedScope
    ) ??
    snapshot?.source_views.find(
      (view) =>
        view.scope.kind === 'workspace' &&
        view.scope.workspace_id === snapshot.workspace_id
    );
  const observation = selectedView;
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Alert
        type="info"
        showIcon
        title={t('ephemeral')}
        description={t('consumed_help')}
      />
      {query.isError && (
        <Alert type="error" showIcon title={t('runtime_error')} />
      )}
      {snapshot && snapshot.source_views.length > 1 && (
        <Select
          aria-label={t('scope')}
          value={selectedView ? scopeKey(selectedView.scope) : undefined}
          options={snapshot.source_views.map((view) => ({
            value: scopeKey(view.scope),
            label: scopeLabel(view.scope)
          }))}
          onChange={navigateEventScope}
        />
      )}
      {snapshot?.source_queue_refresh_error != null && (
        <Alert
          type="warning"
          showIcon
          title={t('source_queue_refresh_error')}
        />
      )}
      {snapshot && observation && (
        <>
          {snapshot.workflow_refresh_error !== null && (
            <Alert
              type="warning"
              showIcon
              title={t('workflow_refresh_error')}
            />
          )}
          {!observation.configured && (
            <Alert type="warning" showIcon title={t('unconfigured')} />
          )}
          <Descriptions
            size="small"
            column={{ xs: 1, sm: 2 }}
            items={[
              {
                key: 'instance_id',
                label: t('instance_id'),
                children: observation.instance_id
              },
              {
                key: 'workspace_id',
                label: t('scope'),
                children: scopeLabel(observation.scope)
              },
              {
                key: 'fetched_at',
                label: t('fetched_at'),
                children: new Date(query.dataUpdatedAt).toLocaleString()
              },
              {
                key: 'observation_ttl_ms',
                label: t('observation_window'),
                children: t('window_ms', {
                  count: observation.observation_ttl_ms
                })
              },
              ...(observation.scope.kind === 'workspace' &&
              observation.scope.workspace_id === snapshot.workspace_id
                ? [
                    {
                      key: 'admission_rejected',
                      label: t('window_rejected'),
                      children: snapshot.admission_rejected
                    }
                  ]
                : []),
              {
                key: 'source_metadata_bytes',
                label: t('source_metadata_bytes'),
                children: observation.source_metadata_bytes
              },
              {
                key: 'observation_bytes',
                label: t('observation_bytes'),
                children: `${observation.observation_bytes} / ${observation.observation_max_bytes}`
              },
              ...(observation.observation_loss.accounting_available
                ? [
                    {
                      key: 'window_started_at_unix_ms',
                      label: t('loss_window_start'),
                      children:
                        observation.observation_loss
                          .window_started_at_unix_ms === null
                          ? '—'
                          : new Date(
                              observation.observation_loss
                                .window_started_at_unix_ms
                            ).toLocaleString()
                    },
                    {
                      key: 'window_duration_ms',
                      label: t('loss_window_duration'),
                      children: t('window_ms', {
                        count: observation.observation_loss.window_duration_ms
                      })
                    },
                    {
                      key: 'expired',
                      label: t('loss_expired'),
                      children: observation.observation_loss.expired
                    },
                    {
                      key: 'evicted',
                      label: t('loss_evicted'),
                      children: observation.observation_loss.evicted
                    },
                    {
                      key: 'oversize_dropped',
                      label: t('loss_oversize_dropped'),
                      children: observation.observation_loss.oversize_dropped
                    }
                  ]
                : [])
            ]}
          />
          <Alert
            type="info"
            title={
              observation.observation_loss.accounting_available
                ? t('loss_window_help')
                : t('loss_unavailable')
            }
          />
        </>
      )}
      {tab === 'queues' ? (
        <>
          {selectedView && <EventSourceQueues sources={selectedView.sources} />}
          {selectedView && selectedView.scope.kind !== 'workspace' && (
            <>
              <Typography.Text strong>
                {t('scope_workflow_queues')}
              </Typography.Text>
              <EventsTable
                tableKey="source-workflow-queues"
                columns={queueColumns}
                rows={selectedView.workflow_queues}
                rowKey="subscription_id"
              />
            </>
          )}
          <Typography.Text strong>
            {t('workspace_workflow_queues')}
          </Typography.Text>
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
            toolbar={
              <Button
                onClick={() => void query.refetch()}
                loading={query.isFetching}
              >
                {t('refresh')}
              </Button>
            }
          />
        </>
      ) : (
        <EventsTable
          tableKey="activity"
          columns={activityColumns}
          rows={selectedView?.activity ?? []}
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
          toolbar={
            <Button
              onClick={() => void query.refetch()}
              loading={query.isFetching}
            >
              {t('refresh')}
            </Button>
          }
        />
      )}
    </Space>
  );
}
