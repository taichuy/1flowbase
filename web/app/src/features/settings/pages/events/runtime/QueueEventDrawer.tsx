import { Button, Empty, Tag, Typography } from 'antd';
import type { DataTableColumn } from '../../../../../shared/ui/data-table/DataTable';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { EventsTable } from '../EventsTable';
import { navigateEventState } from '../event-navigation';
import { RuntimeIdentity } from './RuntimeIdentity';
import { useRuntimeLabels, useWorkflowStatusLabels } from './runtime-labels';
import type {
  ObservedSubscription,
  SubscribedEvent
} from './subscription-view';

export function QueueEventDrawer({
  event,
  onClose
}: {
  event: SubscribedEvent;
  onClose: () => void;
}) {
  const { t, phases } = useRuntimeLabels();
  const { activation, reason } = useWorkflowStatusLabels();
  const subscriptions: DataTableColumn<ObservedSubscription>[] = [
    {
      key: 'consumer',
      title: t('runtime_receiving_owner'),
      width: 230,
      minWidth: 150,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, item) =>
        item.workflow ? (
          <div className="event-runtime-receiver">
            <Tag color="processing">{t('runtime_workflow_consumer')}</Tag>
            <RuntimeIdentity
              label={t('application_id')}
              value={item.workflow.application_id}
            />
          </div>
        ) : (
          <div className="event-runtime-receiver">
            <Tag>{t('runtime_owner_unclassified')}</Tag>
            <Typography.Text type="secondary">
              {t('runtime_unclassified_help')}
            </Typography.Text>
          </div>
        )
    },
    {
      key: 'phase',
      title: t('phase'),
      width: 135,
      render: (_, item) => {
        const phase = item.queue?.trigger_phase ?? item.workflow?.trigger_phase;
        return phase ? phases[phase] : t('unknown');
      }
    },
    {
      key: 'activation',
      title: t('workflow_activation'),
      width: 140,
      render: (_, item) => {
        if (item.workflow)
          return (
            <Typography.Text
              type={
                item.workflow.activation === 'active' ? undefined : 'warning'
              }
              title={
                item.workflow.activation !== 'active'
                  ? reason(item.workflow.reason)
                  : undefined
              }
            >
              {activation(item.workflow.activation)}
            </Typography.Text>
          );
        if (item.queue)
          return item.queue.closed
            ? t('closed')
            : t('runtime_activation_unknown');
        return t('unknown');
      }
    },
    {
      key: 'buffered',
      title: t('buffered'),
      width: 120,
      render: (_, item) => item.queue?.buffered ?? t('unknown')
    },
    {
      key: 'consumer_ids',
      title: t('runtime_consumers'),
      width: 105,
      render: (_, item) =>
        item.queue
          ? t('runtime_consumer_count', {
              count: item.queue.consumer_ids.length
            })
          : t('unknown')
    },
    {
      key: 'details',
      title: t('actions'),
      width: 85,
      mobileRole: 'actions',
      render: (_, item) => (
        <Button
          type="link"
          onClick={() => navigateEventState({ subscription: item.id })}
        >
          {t('view')}
        </Button>
      )
    }
  ];
  return (
    <ResizableDrawer
      open
      title={event.contractId}
      ariaLabel={t('runtime_event_subscriptions')}
      onClose={onClose}
      resizeLabel={t('runtime_resize')}
      defaultWidth={850}
      bodyClassName="event-runtime-details event-runtime-event-drawer"
    >
      <Typography.Paragraph type="secondary">
        {t('runtime_event_subscriptions_help')}
      </Typography.Paragraph>
      <div className="event-runtime-detail-heading">
        <Typography.Title level={5}>
          {t('runtime_center_subscriptions')} · {event.subscriptions.length}
        </Typography.Title>
        <Typography.Text type="secondary">
          {t('runtime_center_queue_only')}
        </Typography.Text>
      </div>
      {event.subscriptions.length ? (
        <EventsTable
          tableKey="event-subscriptions"
          columns={subscriptions}
          rows={event.subscriptions}
          rowKey="id"
          emptyText={t('runtime_no_event_subscriptions')}
        />
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t('runtime_no_event_subscriptions')}
        />
      )}
    </ResizableDrawer>
  );
}
