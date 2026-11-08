import type {
  EventRegisteredSourceSnapshot,
  EventSourceQueueSnapshot
} from '@1flowbase/api-client';
import { Alert, Button, Input, Typography } from 'antd';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { EventsTable } from './EventsTable';
import { navigateEventState, useEventParameter } from './event-navigation';
import { RuntimeDetails } from './runtime/RuntimeDetails';
import { RuntimeIdentity } from './runtime/RuntimeIdentity';
import { timeLabel, useRuntimeLabels } from './runtime/runtime-labels';

export function EventSourceQueues({
  sources
}: {
  sources: EventRegisteredSourceSnapshot[];
}) {
  const { t } = useRuntimeLabels();
  const registration = useEventParameter('source');
  const search = useEventParameter('sources_query') ?? '';
  const selected =
    sources.find((source) => source.registration_id === registration) ??
    (!registration ? sources[0] : undefined);
  const rows = sources.filter((source) =>
    `${source.descriptor.name} ${source.descriptor.source_id} ${source.descriptor.contract_id} ${source.registration_id}`
      .toLocaleLowerCase()
      .includes(search.toLocaleLowerCase())
  );
  const detailKey = useEventParameter('queue');
  const sourceDetails = useEventParameter('source_details') === '1';
  const queue = selected?.queues.find((row) => row.queue_id === detailKey);
  const select = (id: string) =>
    navigateEventState({
      source: id,
      queue: null,
      source_details: null,
      'source-queues_page': null
    });
  const columns: DataTableColumn<EventRegisteredSourceSnapshot>[] = [
    {
      key: 'name',
      title: t('name'),
      width: 210,
      minWidth: 140,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          className="event-runtime-source-button"
          title={row.descriptor.name}
          aria-label={row.descriptor.name}
          aria-pressed={selected?.registration_id === row.registration_id}
          onClick={() => select(row.registration_id)}
        >
          {row.descriptor.name}
        </Button>
      )
    },
    {
      key: 'observed_queues',
      title: t('runtime_observed_queues'),
      width: 100,
      minWidth: 80,
      render: (_, row) => row.queues.length
    },
    {
      key: 'queue_observation_complete',
      title: t('queue_observation'),
      width: 110,
      minWidth: 90,
      render: (_, row) => (
        <Typography.Text
          type={row.queue_observation_complete ? undefined : 'warning'}
        >
          {row.queue_observation_complete
            ? t('queue_observation_complete')
            : t('queue_observation_incomplete')}
        </Typography.Text>
      )
    },
    ...(['source_id', 'point_id', 'contract_id'] as const).map((key) => ({
      key,
      title: t(key),
      width: 220,
      defaultVisibility: 'hidden' as const,
      render: (_: unknown, row: EventRegisteredSourceSnapshot) => (
        <RuntimeIdentity value={row.descriptor[key]} label={t(key)} />
      )
    })),
    {
      key: 'registration_id',
      title: t('registration_id'),
      width: 220,
      defaultVisibility: 'hidden',
      render: (_, row) => (
        <RuntimeIdentity
          value={row.registration_id}
          label={t('registration_id')}
        />
      )
    }
  ];
  const queueColumns: DataTableColumn<EventSourceQueueSnapshot>[] = [
    {
      key: 'queue_id',
      title: t('queue_id'),
      width: 190,
      minWidth: 140,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          className="event-runtime-source-button"
          title={row.queue_id}
          onClick={() => navigateEventState({ queue: row.queue_id })}
        >
          {row.queue_id}
        </Button>
      )
    },
    {
      key: 'buffered',
      title: t('buffered'),
      width: 110,
      render: (_, row) => row.buffered ?? t('unknown')
    },
    {
      key: 'in_flight',
      title: t('in_flight'),
      width: 130,
      render: (_, row) => row.in_flight ?? t('unknown')
    },
    {
      key: 'consumer_ids',
      title: t('runtime_consumers'),
      width: 110,
      render: (_, row) => (
        <Button
          type="link"
          onClick={() => navigateEventState({ queue: row.queue_id })}
        >
          {t('runtime_consumer_count', { count: row.consumer_ids.length })}
        </Button>
      )
    },
    {
      key: 'subscriber_id',
      title: t('original_subscriber_id'),
      width: 220,
      defaultVisibility: 'hidden',
      render: (_, row) => (
        <RuntimeIdentity
          value={row.subscriber_id}
          label={t('original_subscriber_id')}
        />
      )
    },
    ...(
      [
        ['accepted', t('accepted')],
        ['dequeued', t('dequeued')],
        ['dropped', t('dropped')]
      ] as const
    ).map(([key, title]) => ({
      key,
      title,
      width: 130,
      defaultVisibility: 'hidden' as const,
      render: (_: unknown, row: EventSourceQueueSnapshot) =>
        row[key] ?? t('unknown')
    })),
    {
      key: 'observed_at_unix_ms',
      title: t('observed_at'),
      width: 190,
      defaultVisibility: 'hidden',
      render: (_, row) => timeLabel(row.observed_at_unix_ms)
    }
  ];
  return (
    <div className="event-runtime-master-detail">
      <section
        className="event-runtime-source-list"
        aria-label={t('original_sources')}
      >
        <Typography.Title level={5}>{t('original_sources')}</Typography.Title>
        <EventsTable
          urlState
          tableKey="sources"
          rows={rows}
          columns={columns}
          rowKey="registration_id"
          emptyText={search ? t('runtime_no_matches') : t('runtime_no_sources')}
          toolbar={
            <Input.Search
              aria-label={t('runtime_find_source')}
              placeholder={t('runtime_find_source')}
              allowClear
              value={search}
              onChange={(event) =>
                navigateEventState(
                  { sources_query: event.target.value, sources_page: null },
                  true
                )
              }
              onSearch={(value) =>
                navigateEventState({ sources_query: value, sources_page: null })
              }
            />
          }
          rowClassName={(row) =>
            row.registration_id === selected?.registration_id
              ? 'ant-table-row-selected'
              : ''
          }
          onRow={(row) => ({
            'aria-selected': row.registration_id === selected?.registration_id,
            onClick: (event) => {
              if (!(event.target as HTMLElement).closest('button,a,input'))
                select(row.registration_id);
            }
          })}
        />
      </section>
      <section
        className="event-runtime-source-queues"
        aria-label={t('original_queues')}
      >
        <div className="event-runtime-section-heading">
          <div>
            <Typography.Title level={5}>
              {t('original_queues')}
            </Typography.Title>
            <Typography.Text
              className="event-runtime-selection"
              title={selected?.descriptor.name}
            >
              {selected?.descriptor.name ?? t('runtime_choose_source')}
            </Typography.Text>
          </div>
          {selected && (
            <Button
              aria-label={t('view')}
              onClick={() => navigateEventState({ source_details: '1' })}
            >
              {t('view')}
            </Button>
          )}
        </div>
        {registration && !selected && (
          <Alert type="warning" title={t('runtime_source_unavailable')} />
        )}
        {selected && !selected.queue_observation_complete && (
          <Alert
            type="warning"
            showIcon
            title={t('queue_observation_incomplete')}
            description={t('queue_observation_incomplete_help')}
          />
        )}
        {selected ? (
          <EventsTable
            urlState
            key={selected.registration_id}
            tableKey="source-queues"
            columns={queueColumns}
            rows={selected.queues}
            rowKey="queue_id"
            emptyText={
              selected.queue_observation_complete
                ? t('runtime_no_source_queues')
                : t('runtime_incomplete_empty')
            }
          />
        ) : (
          <Typography.Paragraph type="secondary">
            {t('runtime_choose_source')}
          </Typography.Paragraph>
        )}
      </section>
      {(sourceDetails || detailKey) && (
        <RuntimeDetails
          open
          detail={
            sourceDetails && selected
              ? { kind: 'source', value: selected }
              : queue
                ? { kind: 'source_queue', value: queue }
                : undefined
          }
          onClose={() =>
            navigateEventState({ source_details: null, queue: null }, true)
          }
        />
      )}
    </div>
  );
}
