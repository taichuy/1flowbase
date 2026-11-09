import type {
  EventRuntimeActivity,
  EventRegisteredSourceSnapshot
} from '@1flowbase/api-client';
import { Button, Input, Select, Tag, Typography } from 'antd';
import type { ReactNode } from 'react';
import type { DataTableColumn } from '../../../../../shared/ui/data-table/DataTable';
import { EventsTable } from '../EventsTable';
import { navigateEventState, useEventParameter } from '../event-navigation';
import { RuntimeDetails } from './RuntimeDetails';
import { RuntimeIdentity } from './RuntimeIdentity';
import { activityKey, timeLabel, useRuntimeLabels } from './runtime-labels';

export function EventActivity({
  activity,
  sources,
  toolbarActions
}: {
  activity: EventRuntimeActivity[];
  sources: EventRegisteredSourceSnapshot[];
  toolbarActions?: ReactNode;
}) {
  const { t, phases, admissions, scopeLabel } = useRuntimeLabels();
  const search = useEventParameter('query') ?? '',
    phase = useEventParameter('phase'),
    admission = useEventParameter('admission'),
    record = useEventParameter('record');
  const selected = activity.find((row) => activityKey(row) === record);
  const name = (row: EventRuntimeActivity) =>
    sources.find(
      (source) => source.registration_id === row.source_context?.registration_id
    )?.descriptor.name ?? row.contract_id;
  const rows = activity
    .filter(
      (row) =>
        (!phase || row.phase === phase) &&
        (!admission ||
          (admission === 'none'
            ? row.admission === null
            : row.admission === admission)) &&
        [
          name(row),
          row.contract_id,
          row.event_id,
          row.source_event_id,
          row.subscription_id,
          row.consumer_id,
          row.original_subscriber_id,
          row.original_consumer_id,
          ...Object.values(row.source_context ?? {}).flatMap((value) =>
            typeof value === 'string' ? [value] : []
          )
        ].some((value) =>
          value?.toLocaleLowerCase().includes(search.toLocaleLowerCase())
        )
    )
    .sort((a, b) => b.observed_at_unix_ms - a.observed_at_unix_ms);
  const columns: DataTableColumn<EventRuntimeActivity>[] = [
    {
      key: 'observed_at_unix_ms',
      title: t('observed_at'),
      width: 185,
      render: (_, row) => timeLabel(row.observed_at_unix_ms)
    },
    {
      key: 'contract_id',
      title: t('runtime_event'),
      width: 260,
      minWidth: 160,
      sizing: 'fill',
      mobileRole: 'title',
      render: (_, row) => (
        <Button
          type="link"
          className="event-runtime-source-button"
          title={`${name(row)} · ${row.contract_id}`}
          onClick={() => navigateEventState({ record: activityKey(row) })}
        >
          {name(row)}
        </Button>
      )
    },
    {
      key: 'phase',
      title: t('phase'),
      width: 140,
      render: (_, row) => phases[row.phase]
    },
    {
      key: 'admission',
      title: t('admission'),
      width: 180,
      render: (_, row) => (
        <Tag
          color={
            row.admission &&
            ['resource_exhausted', 'invalid_body', 'unconfigured'].includes(
              row.admission
            )
              ? 'warning'
              : undefined
          }
        >
          {row.admission
            ? admissions[row.admission]
            : t('runtime_not_applicable')}
        </Tag>
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
          onClick={() => navigateEventState({ record: activityKey(row) })}
        >
          {t('view')}
        </Button>
      )
    },
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
      title: t(key),
      width: 240,
      defaultVisibility: 'hidden' as const,
      render: (_: unknown, row: EventRuntimeActivity) => (
        <RuntimeIdentity value={row.source_context?.[key]} label={t(key)} />
      )
    })),
    {
      key: 'scope',
      title: t('scope'),
      width: 220,
      defaultVisibility: 'hidden',
      render: (_, row) =>
        row.source_context ? scopeLabel(row.source_context.scope, true) : '—'
    },
    ...(
      [
        'event_id',
        'source_event_id',
        'subscription_id',
        'consumer_id',
        'original_subscriber_id',
        'original_consumer_id'
      ] as const
    ).map((key) => ({
      key,
      title: t(key),
      width: 240,
      defaultVisibility: 'hidden' as const,
      render: (_: unknown, row: EventRuntimeActivity) => (
        <RuntimeIdentity value={row[key]} label={t(key)} />
      )
    }))
  ];
  const set = (values: Record<string, string | null>) =>
    navigateEventState({ ...values, activity_page: null, record: null });
  return (
    <div className="event-runtime-activity">
      <Typography.Text type="secondary">
        {t('runtime_snapshot_filter')}
      </Typography.Text>
      <EventsTable
        toolbar={toolbarActions}
        toolbarStart={
          <div className="event-runtime-filters">
            <Input.Search
              className="event-runtime-search"
              aria-label={t('runtime_find_activity')}
              placeholder={t('runtime_find_activity')}
              allowClear
              value={search}
              onChange={(event) =>
                navigateEventState(
                  {
                    query: event.target.value,
                    activity_page: null,
                    record: null
                  },
                  true
                )
              }
              onSearch={(value) => set({ query: value })}
            />
            <Select
              className="event-runtime-filter-select"
              aria-label={t('phase')}
              value={phase ?? undefined}
              placeholder={t('runtime_all_phases')}
              allowClear
              options={Object.entries(phases).map(([value, label]) => ({
                value,
                label
              }))}
              onChange={(value) => set({ phase: value ?? null })}
            />
            <Select
              className="event-runtime-filter-select"
              aria-label={t('admission')}
              value={admission ?? undefined}
              placeholder={t('runtime_all_admissions')}
              allowClear
              options={[
                ...Object.entries(admissions).map(([value, label]) => ({
                  value,
                  label
                })),
                { value: 'none', label: t('runtime_not_applicable') }
              ]}
              onChange={(value) => set({ admission: value ?? null })}
            />
            {(search || phase || admission) && (
              <Button
                onClick={() =>
                  set({ query: null, phase: null, admission: null })
                }
              >
                {t('runtime_reset')}
              </Button>
            )}
          </div>
        }
        mobileRecords={JSON.stringify([search, phase, admission])}
        urlState
        tableKey="activity"
        columns={columns}
        rows={rows}
        rowKey={activityKey}
        emptyText={
          activity.length ? t('runtime_no_matches') : t('runtime_no_activity')
        }
      />
      {record && (
        <RuntimeDetails
          open
          detail={selected ? { kind: 'activity', value: selected } : undefined}
          onClose={() => navigateEventState({ record: null }, true)}
        />
      )}
    </div>
  );
}
