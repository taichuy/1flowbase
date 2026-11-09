import type {
  EventRuntimeActivity,
  EventRegisteredSourceSnapshot,
  EventSourceQueueSnapshot,
  EventRuntimeQueueSnapshot,
  EventSourceRuntimeSnapshot,
  EventRuntimeSnapshot
} from '@1flowbase/api-client';
import { Alert, Descriptions, Typography } from 'antd';
import type { DescriptionsProps } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { RuntimeIdentity } from './RuntimeIdentity';
import { timeLabel, useRuntimeLabels } from './runtime-labels';

export type RuntimeDetail =
  | { kind: 'activity'; value: EventRuntimeActivity }
  | { kind: 'source'; value: EventRegisteredSourceSnapshot }
  | { kind: 'source_queue'; value: EventSourceQueueSnapshot }
  | { kind: 'workflow_queue'; value: EventRuntimeQueueSnapshot };

export function RuntimeDetails({
  detail,
  open,
  onClose,
  observation,
  snapshot,
  fetchedAt
}: {
  detail?: RuntimeDetail;
  open: boolean;
  onClose: () => void;
  observation?: EventSourceRuntimeSnapshot;
  snapshot?: EventRuntimeSnapshot;
  fetchedAt?: number;
}) {
  // A URL-backed drawer is unmounted when Back or Escape closes it. Preserve its
  // trigger before the shared drawer moves focus into the dialog.
  const [returnFocus] = useState(() =>
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
  );
  useEffect(
    () => () => {
      window.requestAnimationFrame(() => {
        if (returnFocus?.isConnected && returnFocus !== document.body)
          returnFocus.focus();
        else
          document
            .querySelector<HTMLElement>('.event-runtime-toolbar button')
            ?.focus();
      });
    },
    [returnFocus]
  );
  const { t, phases, admissions, scopeLabel } = useRuntimeLabels();
  const fields: NonNullable<DescriptionsProps['items']> = [];
  const add = (key: string, label: string, children: ReactNode) =>
    fields.push({ key, label, children });
  const identity = (
    key: string,
    value: string | null | undefined,
    label = t(key)
  ) => add(key, label, <RuntimeIdentity label={label} value={value} full />);
  let title = t('runtime_observation_details');
  if (detail?.kind === 'activity') {
    title = t('runtime_activity_details');
    const row = detail.value;
    add('observed_at', t('observed_at'), timeLabel(row.observed_at_unix_ms));
    identity('contract_id', row.contract_id);
    add('phase', t('phase'), phases[row.phase]);
    add(
      'admission',
      t('admission'),
      row.admission ? admissions[row.admission] : t('runtime_not_applicable')
    );
    identity('event_id', row.event_id);
    identity('workspace_id', row.workspace_id);
    identity('source_event_id', row.source_event_id);
    identity('subscription_id', row.subscription_id);
    identity('consumer_id', row.consumer_id);
    identity('original_subscriber_id', row.original_subscriber_id);
    identity('original_consumer_id', row.original_consumer_id);
    const source = row.source_context;
    if (source) {
      add('scope', t('scope'), scopeLabel(source.scope, true));
      identity('source_id', source.source_id);
      identity('point_id', source.point_id);
      identity('registration_id', source.registration_id);
      identity('original_event_id', source.original_event_id);
      identity('producer_id', source.producer_id);
      for (const [key, value] of Object.entries(source.attributes ?? {}))
        add(
          `attribute:${key}`,
          key,
          <RuntimeIdentity value={value} label={key} full />
        );
    }
  } else if (detail?.kind === 'source') {
    title = detail.value.descriptor.name;
    const row = detail.value;
    const d = row.descriptor;
    add('description', t('description'), d.description || '—');
    identity('source_id', d.source_id);
    identity('point_id', d.point_id);
    identity('contract_id', d.contract_id);
    identity('registration_id', row.registration_id);
    identity('owner_id', d.owner_id);
    add('scope', t('scope'), scopeLabel(d.scope, true));
    add(
      'complete',
      t('queue_observation'),
      row.queue_observation_complete
        ? t('queue_observation_complete')
        : t('queue_observation_incomplete')
    );
    add('metadata', t('source_metadata_bytes'), row.metadata_bytes);
    add(
      'phases',
      t('supported_phases'),
      d.supported_phases.map((p) => phases[p]).join(' / ') || t('unsupported')
    );
  } else if (detail?.kind === 'source_queue') {
    title = t('runtime_queue_details');
    const row = detail.value;
    identity('queue_id', row.queue_id);
    identity('original_subscriber_id', row.subscriber_id);
    add(
      'consumer_ids',
      t('consumer_ids'),
      row.consumer_ids.length
        ? row.consumer_ids.map((id) => (
            <RuntimeIdentity
              key={id}
              value={id}
              label={t('consumer_id')}
              full
            />
          ))
        : t('no_consumer')
    );
    for (const [key, label] of [
      ['buffered', t('buffered')],
      ['in_flight', t('in_flight')],
      ['accepted', t('accepted')],
      ['dequeued', t('dequeued')],
      ['dropped', t('dropped')]
    ] as const)
      add(key, label, row[key] ?? t('unknown'));
    add('observed_at', t('observed_at'), timeLabel(row.observed_at_unix_ms));
  } else if (detail?.kind === 'workflow_queue') {
    title = t('runtime_queue_details');
    const row = detail.value;
    identity('contract_id', row.contract_id);
    identity('subscription_id', row.subscription_id);
    identity('workspace_id', row.workspace_id);
    add('phase', t('phase'), phases[row.trigger_phase]);
    add('queue_state', t('queue_state'), row.closed ? t('closed') : t('open'));
    add(
      'consumer_ids',
      t('consumer_ids'),
      row.consumer_ids.length
        ? row.consumer_ids.map((id) => (
            <RuntimeIdentity
              key={id}
              value={id}
              label={t('consumer_id')}
              full
            />
          ))
        : t('no_consumer')
    );
    for (const [key, label] of [
      ['buffered', t('buffered')],
      ['in_flight', t('in_flight')],
      ['accepted', t('accepted')],
      ['consumed', t('dequeued')],
      ['admission_rejected', t('admission_rejected')]
    ] as const)
      add(key, label, row[key]);
    if (row.source_target) {
      identity('source_id', row.source_target.source_id);
      identity('application_id', row.source_target.workflow_id);
      add('scope', t('scope'), scopeLabel(row.source_target.scope, true));
      for (const [key, value] of Object.entries(
        row.source_target.filters ?? {}
      ))
        add(
          `filter:${key}`,
          key,
          <RuntimeIdentity value={value} label={key} full />
        );
    }
  } else if (observation) {
    identity('instance_id', observation.instance_id);
    add('scope', t('scope'), scopeLabel(observation.scope, true));
    add('fetched_at', t('fetched_at'), fetchedAt ? timeLabel(fetchedAt) : '—');
    add(
      'observation_window',
      t('observation_window'),
      t('window_ms', { count: observation.observation_ttl_ms })
    );
    add(
      'source_metadata_bytes',
      t('source_metadata_bytes'),
      observation.source_metadata_bytes
    );
    add(
      'observation_bytes',
      t('observation_bytes'),
      `${observation.observation_bytes} / ${observation.observation_max_bytes}`
    );
    if (
      observation.scope.kind === 'workspace' &&
      observation.scope.workspace_id === snapshot?.workspace_id
    )
      add('window_rejected', t('window_rejected'), snapshot.admission_rejected);
    const loss = observation.observation_loss;
    if (loss.accounting_available) {
      add(
        'loss_window_start',
        t('loss_window_start'),
        loss.window_started_at_unix_ms == null
          ? '—'
          : timeLabel(loss.window_started_at_unix_ms)
      );
      add(
        'loss_window_duration',
        t('loss_window_duration'),
        t('window_ms', { count: loss.window_duration_ms })
      );
      add('expired', t('loss_expired'), loss.expired);
      add('evicted', t('loss_evicted'), loss.evicted);
      add('oversize', t('loss_oversize_dropped'), loss.oversize_dropped);
    }
  }
  return (
    <ResizableDrawer
      open={open}
      title={title}
      ariaLabel={title}
      onClose={onClose}
      resizeLabel={t('runtime_resize')}
      defaultWidth={680}
      bodyClassName="event-runtime-details"
    >
      <Descriptions size="small" column={1} items={fields} />
      {!detail && !observation ? (
        <Alert type="warning" title={t('runtime_record_unavailable')} />
      ) : null}
      <Typography.Paragraph type="secondary">
        {t('ephemeral')}
      </Typography.Paragraph>
      <Typography.Paragraph type="secondary">
        {t('consumed_help')}
      </Typography.Paragraph>
      {detail?.kind === 'source_queue' || detail?.kind === 'workflow_queue' ? (
        <Typography.Paragraph type="secondary">
          {t('queue_counters_help')}
        </Typography.Paragraph>
      ) : null}
      {!detail && observation ? (
        <Typography.Paragraph type="secondary">
          {observation.observation_loss.accounting_available
            ? t('loss_window_help')
            : t('loss_unavailable')}
        </Typography.Paragraph>
      ) : null}
    </ResizableDrawer>
  );
}
