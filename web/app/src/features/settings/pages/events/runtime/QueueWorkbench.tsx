import type {
  EventRuntimeQueueSnapshot,
  EventWorkflowStatus
} from '@1flowbase/api-client';
import { Alert, Button, Input, Tag, Typography } from 'antd';
import type { ReactNode } from 'react';
import type { DataTableColumn } from '../../../../../shared/ui/data-table/DataTable';
import { EventsTable } from '../EventsTable';
import { EventWorkflowSubscriptions } from '../EventWorkflowSubscriptions';
import { navigateEventState, useEventParameter } from '../event-navigation';
import { QueueEventDrawer } from './QueueEventDrawer';
import { RuntimeDetails } from './RuntimeDetails';
import { useRuntimeLabels } from './runtime-labels';
import {
  buildSubscribedEvents,
  type SubscribedEvent
} from './subscription-view';

/**
 * Operator view: event -> independent subscriptions -> consumers.
 *
 * The primary table is NOT a list of registered sources or event definitions.
 * It projects runtime memberships and configured workflow activation statuses.
 * Source-owned queue observations are not event-center subscriptions.
 */
export function QueueWorkbench({
  queues,
  workflows,
  toolbarActions
}: {
  queues: EventRuntimeQueueSnapshot[];
  workflows: EventWorkflowStatus[];
  toolbarActions?: ReactNode;
}) {
  const { t } = useRuntimeLabels();
  const search = useEventParameter('queue_query') ?? '';
  const eventId = useEventParameter('event');
  const subscriptionId = useEventParameter('subscription');
  const { events, unclassified } = buildSubscribedEvents({
    queues,
    workflows
  });
  const rows = events.filter((row) => {
    const query = search.trim().toLocaleLowerCase();
    return (
      !query ||
      row.contractId.toLocaleLowerCase().includes(query) ||
      row.subscriptions.some((subscription) =>
        [
          subscription.id,
          subscription.workflow?.application_id,
          subscription.queue?.source_target?.source_id
        ].some((field) => field?.toLocaleLowerCase().includes(query))
      )
    );
  });
  const selectedEvent = events.find((row) => row.contractId === eventId);
  const selectedSubscription = selectedEvent?.subscriptions.find(
    (row) => row.id === subscriptionId
  );
  const openEvent = (row: SubscribedEvent) =>
    navigateEventState({
      event: row.contractId,
      subscription: null
    });
  const columns: DataTableColumn<SubscribedEvent>[] = [
    {
      key: 'contract_id',
      title: t('runtime_event'),
      width: 350,
      minWidth: 200,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          className="event-runtime-source-button"
          title={row.contractId}
          onClick={() => openEvent(row)}
        >
          {row.contractId}
        </Button>
      )
    },
    {
      key: 'subscriptions',
      title: t('runtime_center_subscriptions'),
      width: 135,
      render: (_, row) => row.subscriptions.length
    },
    {
      key: 'buffered',
      title: t('runtime_center_buffered'),
      width: 155,
      render: (_, row) => row.buffered ?? t('unknown')
    },
    {
      key: 'attention',
      title: t('runtime_subscription_health'),
      width: 145,
      render: (_, row) =>
        row.needsAttention ? (
          <Tag color="warning">
            {t('runtime_attention_count', { count: row.needsAttention })}
          </Tag>
        ) : (
          <Typography.Text type="secondary">
            {t('runtime_no_known_issues')}
          </Typography.Text>
        )
    },
    {
      key: 'details',
      title: t('actions'),
      width: 90,
      mobileRole: 'actions',
      render: (_, row) => (
        <Button type="link" onClick={() => openEvent(row)}>
          {t('view')}
        </Button>
      )
    }
  ];
  return (
    <div className="event-runtime-workbench">
      <EventsTable
        urlState
        tableKey="event-groups"
        columns={columns}
        rows={rows}
        rowKey="contractId"
        emptyText={
          search ? t('runtime_no_matches') : t('runtime_no_subscribed_events')
        }
        toolbar={toolbarActions}
        toolbarStart={
          <Input.Search
            className="event-runtime-event-search"
            allowClear
            aria-label={t('runtime_find_subscribed_event')}
            placeholder={t('runtime_find_subscribed_event')}
            value={search}
            onChange={(event) =>
              navigateEventState(
                {
                  queue_query: event.target.value,
                  'event-groups_page': null
                },
                true
              )
            }
            onSearch={(value) =>
              navigateEventState({
                queue_query: value,
                'event-groups_page': null
              })
            }
          />
        }
        rowClassName={() => 'event-runtime-event-row'}
        onRow={(row) => ({
          onClick: (event) => {
            if (!(event.target as HTMLElement).closest('button,a,input'))
              openEvent(row);
          }
        })}
      />
      {unclassified.length > 0 && (
        <>
          <Alert
            type="warning"
            showIcon
            title={t('runtime_unclassified_subscriptions', {
              count: unclassified.length
            })}
            description={t('runtime_unclassified_status_help')}
          />
          <EventWorkflowSubscriptions subscriptions={unclassified} />
        </>
      )}
      {eventId && !subscriptionId && selectedEvent && (
        <QueueEventDrawer
          event={selectedEvent}
          onClose={() =>
            navigateEventState({ event: null, subscription: null }, true)
          }
        />
      )}
      {eventId && !subscriptionId && !selectedEvent && (
        <RuntimeDetails
          open
          onClose={() => navigateEventState({ event: null }, true)}
        />
      )}
      {subscriptionId && (
        <RuntimeDetails
          open
          detail={
            selectedSubscription
              ? { kind: 'subscription', value: selectedSubscription }
              : undefined
          }
          onClose={() => navigateEventState({ subscription: null }, true)}
        />
      )}
    </div>
  );
}
