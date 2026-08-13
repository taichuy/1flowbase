import { Empty, Tag, Typography } from 'antd';

import type { ApplicationRunResumeTimelineSummary as ApplicationRunResumeTimelineData } from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import './application-run-detail-panel.css';

const RESUME_EVENT_TYPES = new Set([
  'public_run_resume_requested',
  'public_run_resume_succeeded',
  'public_run_resume_failed',
  'public_run_resume_cancelled',
  'flow_run_resumed'
]);

interface ResumeTimelineItem {
  key: string;
  occurredAt: string;
  title: string;
  status: string;
  color: string;
  description: string | null;
}

function eventStatusColor(eventType: string) {
  if (eventType.endsWith('_failed')) {
    return 'red';
  }
  if (eventType.endsWith('_succeeded') || eventType === 'flow_run_resumed') {
    return 'green';
  }
  return 'default';
}

function resumeEventLabel(eventType: string) {
  switch (eventType) {
    case 'public_run_resume_requested':
      return i18nText('applications', 'auto.resume_event_requested');
    case 'public_run_resume_succeeded':
      return i18nText('applications', 'auto.resume_event_succeeded');
    case 'public_run_resume_failed':
      return i18nText('applications', 'auto.resume_event_failed');
    case 'public_run_resume_cancelled':
      return i18nText('applications', 'auto.resume_event_cancelled');
    case 'flow_run_resumed':
      return i18nText('applications', 'auto.resume_event_resumed');
    default:
      return eventType;
  }
}

function callbackStatusColor(status: string) {
  switch (status) {
    case 'pending':
      return 'gold';
    case 'completed':
      return 'green';
    case 'cancelled':
      return 'default';
    default:
      return 'blue';
  }
}

function callbackStatusLabel(status: string) {
  switch (status) {
    case 'pending':
      return i18nText('applications', 'auto.callback_status_pending');
    case 'completed':
      return i18nText('applications', 'auto.callback_status_completed');
    case 'cancelled':
      return i18nText('applications', 'auto.callback_status_cancelled');
    default:
      return status;
  }
}

function callbackKindLabel(callbackKind: string) {
  switch (callbackKind) {
    case 'llm_tool_calls':
      return i18nText('applications', 'auto.callback_kind_llm_tool_calls');
    case 'external_callback':
      return i18nText('applications', 'auto.callback_kind_external_callback');
    case 'data_model_side_effect_confirmation':
      return i18nText(
        'applications',
        'auto.callback_kind_data_model_side_effect_confirmation'
      );
    default:
      return callbackKind;
  }
}

function buildResumeTimeline(
  detail: ApplicationRunResumeTimelineData
): ResumeTimelineItem[] {
  const eventItems: ResumeTimelineItem[] = [];
  for (const event of detail.events) {
    if (!RESUME_EVENT_TYPES.has(event.event_type)) {
      continue;
    }

    eventItems.push({
      key: `event-${event.id}`,
      occurredAt: event.created_at,
      title: resumeEventLabel(event.event_type),
      status: i18nText('applications', 'auto.resume_timeline_event'),
      color: eventStatusColor(event.event_type),
      description: event.description
    });
  }
  const callbackItems = detail.callback_tasks.map((task) => ({
    key: `callback-${task.id}-${task.status}`,
    occurredAt: task.completed_at ?? task.created_at,
    title: callbackKindLabel(task.callback_kind),
    status: callbackStatusLabel(task.status),
    color: callbackStatusColor(task.status),
    description: task.id
  }));

  return [...eventItems, ...callbackItems].sort((left, right) =>
    left.occurredAt.localeCompare(right.occurredAt)
  );
}

export function ApplicationRunResumeTimeline({
  detail
}: {
  detail: ApplicationRunResumeTimelineData | null;
}) {
  const items = detail ? buildResumeTimeline(detail) : [];

  return (
    <section className="application-run-detail__timeline">
      <div className="application-run-detail__timeline-header">
        <Typography.Text strong style={{ fontSize: 13 }}>
          {i18nText('applications', 'auto.resume_timeline')}
        </Typography.Text>
        {detail ? <Tag>{detail.flow_run_status}</Tag> : null}
      </div>
      {items.length > 0 ? (
        <div className="application-run-detail__timeline-list">
          {items.map((item) => (
            <div
              className="application-run-detail__timeline-item"
              key={item.key}
            >
              <div className="application-run-detail__timeline-main">
                <Typography.Text style={{ fontSize: 12 }}>
                  {item.title}
                </Typography.Text>
                <Tag color={item.color} style={{ marginInlineEnd: 0 }}>
                  {item.status}
                </Tag>
              </div>
              <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                {new Date(item.occurredAt).toLocaleString()}
              </Typography.Text>
              {item.description ? (
                <Typography.Text code style={{ fontSize: 12 }}>
                  {item.description.slice(0, 32)}
                </Typography.Text>
              ) : null}
            </div>
          ))}
        </div>
      ) : (
        <Empty
          description={i18nText('applications', 'auto.no_resume_events')}
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      )}
    </section>
  );
}
