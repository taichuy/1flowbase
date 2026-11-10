import type { EventRuntimeActivity } from '@1flowbase/api-client';
import { Alert, Descriptions, Typography } from 'antd';
import type { DescriptionsProps } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';
import { ResizableDrawer } from '../../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { RuntimeIdentity } from './RuntimeIdentity';
import type { ObservedSubscription } from './subscription-view';
import {
  timeLabel,
  useRuntimeLabels,
  useWorkflowStatusLabels
} from './runtime-labels';

export type RuntimeDetail =
  | { kind: 'activity'; value: EventRuntimeActivity }
  | { kind: 'subscription'; value: ObservedSubscription };

export function RuntimeDetails({
  detail,
  open,
  onClose
}: {
  detail?: RuntimeDetail;
  open: boolean;
  onClose: () => void;
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
            .querySelector<HTMLElement>('[data-event-runtime-refresh]')
            ?.focus();
      });
    },
    [returnFocus]
  );
  const { t, phases, admissions, scopeLabel } = useRuntimeLabels();
  const { activation, reason } = useWorkflowStatusLabels();
  const fields: NonNullable<DescriptionsProps['items']> = [];
  const add = (key: string, label: string, children: ReactNode) =>
    fields.push({ key, label, children });
  const identity = (
    key: string,
    value: string | null | undefined,
    label = t(key)
  ) => add(key, label, <RuntimeIdentity label={label} value={value} full />);
  let title = t('view');
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
  } else if (detail?.kind === 'subscription') {
    title = t('runtime_subscription_details');
    const row = detail.value;
    const workflow = row.workflow;
    const queue = row.queue;
    identity('contract_id', row.contractId);
    identity('subscription_id', row.id);
    if (workflow) {
      add(
        'owner_type',
        t('runtime_receiving_owner'),
        t('runtime_workflow_consumer')
      );
      identity('application_id', workflow.application_id);
      add(
        'activation',
        t('workflow_activation'),
        activation(workflow.activation)
      );
      if (workflow.activation !== 'active')
        add('reason', t('workflow_reason'), reason(workflow.reason));
    } else {
      add(
        'owner_type',
        t('runtime_receiving_owner'),
        t('runtime_owner_unclassified')
      );
    }
    const phase = queue?.trigger_phase ?? workflow?.trigger_phase;
    add('phase', t('phase'), phase ? phases[phase] : t('unknown'));
    if (queue) {
      add(
        'queue_state',
        t('queue_state'),
        queue.closed ? t('closed') : t('open')
      );
      add(
        'consumer_ids',
        t('consumer_ids'),
        queue.consumer_ids.length
          ? queue.consumer_ids.map((id) => (
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
        add(key, label, queue[key]);
    } else {
      add(
        'queue_unobserved',
        t('runtime_queue_state'),
        t('runtime_queue_not_observed')
      );
    }
    const source = queue?.source_target ?? workflow?.source;
    if (source) {
      identity('source_id', source.source_id);
      add('scope', t('scope'), scopeLabel(source.scope, true));
      for (const [key, value] of Object.entries(source.filters ?? {}))
        add(
          `filter:${key}`,
          key,
          <RuntimeIdentity value={value} label={key} full />
        );
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
      {detail && <Descriptions size="small" column={1} items={fields} />}
      {!detail && (
        <Alert type="warning" title={t('runtime_record_unavailable')} />
      )}
      <Typography.Paragraph type="secondary">
        {t('ephemeral')}
      </Typography.Paragraph>
      <Typography.Paragraph type="secondary">
        {t('consumed_help')}
      </Typography.Paragraph>
      {detail?.kind === 'subscription' ? (
        <Typography.Paragraph type="secondary">
          {t('queue_counters_help')}
        </Typography.Paragraph>
      ) : null}
    </ResizableDrawer>
  );
}
