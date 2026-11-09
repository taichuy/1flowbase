import type { EventRuntimeQueueSnapshot } from '@1flowbase/api-client';
import { Button, Typography } from 'antd';
import type { DataTableColumn } from '../../../../../shared/ui/data-table/DataTable';
import { EventsTable } from '../EventsTable';
import { navigateEventState, useEventParameter } from '../event-navigation';
import { RuntimeDetails } from './RuntimeDetails';
import { RuntimeIdentity } from './RuntimeIdentity';
import { useRuntimeLabels } from './runtime-labels';

export function WorkflowQueues({
  queues
}: {
  queues: EventRuntimeQueueSnapshot[];
}) {
  const { t, phases, scopeLabel } = useRuntimeLabels();
  const id = useEventParameter('workflow_queue');
  const selected = queues.find((row) => row.subscription_id === id);
  const columns: DataTableColumn<EventRuntimeQueueSnapshot>[] = [
    {
      key: 'contract_id',
      title: t('contract_id'),
      width: 240,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          className="event-runtime-source-button"
          title={row.contract_id}
          onClick={() =>
            navigateEventState({ workflow_queue: row.subscription_id })
          }
        >
          {row.contract_id}
        </Button>
      )
    },
    {
      key: 'closed',
      title: t('queue_state'),
      width: 110,
      render: (_, row) => (row.closed ? t('closed') : t('open'))
    },
    {
      key: 'buffered',
      title: t('buffered'),
      width: 110,
      dataIndex: 'buffered'
    },
    {
      key: 'in_flight',
      title: t('in_flight'),
      width: 130,
      dataIndex: 'in_flight'
    },
    {
      key: 'consumer_ids',
      title: t('runtime_consumers'),
      width: 100,
      render: (_, row) => (
        <Button
          type="link"
          onClick={() =>
            navigateEventState({ workflow_queue: row.subscription_id })
          }
        >
          {t('runtime_consumer_count', { count: row.consumer_ids.length })}
        </Button>
      )
    },
    {
      key: 'details',
      title: t('actions'),
      width: 90,
      minWidth: 80,
      mobileRole: 'actions',
      render: (_, row) => (
        <Button
          type="link"
          onClick={() =>
            navigateEventState({ workflow_queue: row.subscription_id })
          }
        >
          {t('view')}
        </Button>
      )
    },
    {
      key: 'subscription_id',
      title: t('subscription_id'),
      width: 220,
      defaultVisibility: 'hidden',
      render: (_, row) => (
        <RuntimeIdentity
          value={row.subscription_id}
          label={t('subscription_id')}
        />
      )
    },
    {
      key: 'source_target',
      title: t('source_target'),
      width: 300,
      defaultVisibility: 'hidden',
      render: (_, row) =>
        row.source_target ? (
          <RuntimeIdentity
            label={t('source_target')}
            value={[
              scopeLabel(row.source_target.scope, true),
              row.source_target.source_id,
              ...Object.entries(row.source_target.filters ?? {}).map(
                ([key, value]) => `${key}=${value}`
              ),
              row.source_target.workflow_id
            ].join(' · ')}
          />
        ) : (
          '—'
        )
    },
    {
      key: 'trigger_phase',
      title: t('phase'),
      width: 140,
      defaultVisibility: 'hidden',
      render: (_, row) => phases[row.trigger_phase]
    },
    {
      key: 'accepted',
      title: t('accepted'),
      width: 110,
      dataIndex: 'accepted',
      defaultVisibility: 'hidden'
    },
    {
      key: 'consumed',
      title: t('dequeued'),
      width: 110,
      dataIndex: 'consumed',
      defaultVisibility: 'hidden'
    },
    {
      key: 'admission_rejected',
      title: t('admission_rejected'),
      width: 130,
      dataIndex: 'admission_rejected',
      defaultVisibility: 'hidden'
    }
  ];
  return (
    <>
      <Typography.Paragraph type="secondary">
        {t('queue_counters_help')}
      </Typography.Paragraph>
      <EventsTable
        urlState
        tableKey="queues"
        rows={queues}
        columns={columns}
        rowKey="subscription_id"
        emptyText={t('runtime_no_workflow_queues')}
      />
      {id && (
        <RuntimeDetails
          open
          detail={
            selected ? { kind: 'workflow_queue', value: selected } : undefined
          }
          onClose={() => navigateEventState({ workflow_queue: null }, true)}
        />
      )}
    </>
  );
}
