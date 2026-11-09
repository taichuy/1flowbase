import { ApiClientError, getEventRuntime } from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { Alert, Button, Collapse, Select, Spin, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useAuthStore } from '../../../../state/auth-store';
import { EventSourceQueues } from './EventSourceQueues';
import {
  navigateEventScope,
  navigateEventState,
  useEventParameter,
  useEventScope
} from './event-navigation';
import { EventWorkflowSubscriptions } from './EventWorkflowSubscriptions';
import { EventActivity } from './runtime/EventActivity';
import { RuntimeDetails } from './runtime/RuntimeDetails';
import { WorkflowQueues } from './runtime/WorkflowQueues';
import {
  scopeKey,
  timeLabel,
  useRuntimeLabels
} from './runtime/runtime-labels';
import './runtime/event-runtime.css';

export function EventRuntimePanel({ tab }: { tab: 'queues' | 'activity' }) {
  const { t, scopeLabel } = useRuntimeLabels();
  const requestedScope = useEventScope();
  const diagnostics = useEventParameter('observation') === '1';
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const query = useQuery({
    queryKey: ['settings', 'events', 'runtime', workspaceId],
    queryFn: () => getEventRuntime(),
    gcTime: 0,
    retry: false,
    refetchOnWindowFocus: false
  });
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 15000);
    return () => window.clearInterval(timer);
  }, []);
  const snapshot = query.data;
  const selectedView = requestedScope
    ? snapshot?.source_views.find(
        (view) => scopeKey(view.scope) === requestedScope
      )
    : snapshot?.source_views.find(
        (view) =>
          view.scope.kind === 'workspace' &&
          view.scope.workspace_id === snapshot.workspace_id
      );
  const denied =
    query.error instanceof ApiClientError &&
    [401, 403].includes(query.error.status);
  // A rejected authorization response must not continue displaying an older privileged snapshot.
  const observation = denied ? undefined : selectedView;
  const stale =
    observation &&
    Math.max(now, Date.now()) - query.dataUpdatedAt >=
      observation.observation_ttl_ms;
  const workspaceView =
    observation?.scope.kind === 'workspace' &&
    observation.scope.workspace_id === snapshot?.workspace_id;
  const needsAttention = workspaceView
    ? (snapshot?.workflow_subscriptions.filter(
        (row) => row.activation !== 'active'
      ) ?? [])
    : [];
  const refresh = () => void query.refetch();
  return (
    <div className="event-runtime" aria-busy={query.isFetching}>
      <div className="event-runtime-toolbar">
        <Select
          className="event-runtime-scope"
          aria-label={t('scope')}
          value={
            observation
              ? scopeKey(observation.scope)
              : (requestedScope ?? undefined)
          }
          placeholder={t('scope')}
          options={
            denied
              ? []
              : snapshot?.source_views.map((view) => ({
                  value: scopeKey(view.scope),
                  label: scopeLabel(view.scope)
                }))
          }
          onChange={navigateEventScope}
          popupMatchSelectWidth={false}
        />
        <Typography.Text
          type="secondary"
          className="event-runtime-freshness"
          role="status"
        >
          {query.isFetching
            ? t('runtime_refreshing')
            : query.dataUpdatedAt
              ? t('runtime_snapshot_time', {
                  time: timeLabel(query.dataUpdatedAt)
                })
              : t('runtime_loading')}
        </Typography.Text>
        <div className="event-runtime-toolbar-actions">
          <Button
            aria-label={t('refresh')}
            onClick={refresh}
            loading={query.isFetching}
            icon={<ReloadOutlined />}
          >
            {t('refresh')}
          </Button>
          <Button
            disabled={!observation}
            onClick={() => navigateEventState({ observation: '1' })}
          >
            {t('runtime_observation_details')}
          </Button>
        </div>
      </div>
      <Typography.Text type="secondary">
        {t('runtime_snapshot_note')}
      </Typography.Text>
      {query.isError && (
        <Alert
          type="error"
          showIcon
          title={denied ? t('runtime_forbidden') : t('runtime_error')}
          description={
            !denied && snapshot
              ? t('runtime_retained_snapshot', {
                  time: timeLabel(query.dataUpdatedAt)
                })
              : undefined
          }
          action={
            <Button
              aria-label={t('runtime_retry')}
              size="small"
              onClick={refresh}
              loading={query.isFetching}
            >
              {t('runtime_retry')}
            </Button>
          }
        />
      )}
      {!query.isError && stale && (
        <Alert type="warning" showIcon title={t('runtime_stale_snapshot')} />
      )}
      {snapshot && !denied && snapshot.source_queue_refresh_error !== null && (
        <Alert
          type="warning"
          showIcon
          title={t('source_queue_refresh_error')}
        />
      )}
      {workspaceView && snapshot?.workflow_refresh_error !== null && (
        <Alert type="warning" showIcon title={t('workflow_refresh_error')} />
      )}
      {observation && !observation.configured && (
        <Alert type="warning" showIcon title={t('unconfigured')} />
      )}
      {snapshot && !denied && !observation && (
        <Alert type="warning" showIcon title={t('runtime_scope_unavailable')} />
      )}
      {observation && !observation.observation_loss.accounting_available && (
        <Alert type="info" showIcon title={t('loss_unavailable')} />
      )}
      {observation?.observation_loss.accounting_available &&
        (observation.observation_loss.evicted > 0 ||
          observation.observation_loss.oversize_dropped > 0) && (
          <Alert type="warning" showIcon title={t('runtime_records_limited')} />
        )}
      {query.isPending ? (
        <Spin description={t('runtime_loading')}>
          <div className="event-runtime-loading" />
        </Spin>
      ) : (
        observation &&
        (tab === 'activity' ? (
          <EventActivity
            key={scopeKey(observation.scope)}
            activity={observation.activity}
            sources={observation.sources}
          />
        ) : (
          <>
            <EventSourceQueues
              key={scopeKey(observation.scope)}
              sources={observation.sources}
            />
            {needsAttention.length > 0 && (
              <EventWorkflowSubscriptions
                subscriptions={snapshot!.workflow_subscriptions}
              />
            )}
            <Collapse
              ghost
              defaultActiveKey={
                observation.sources.length === 0 ||
                observation.workflow_queues.length > 0
                  ? ['workflows']
                  : []
              }
              items={[
                {
                  key: 'workflows',
                  label: `${t(workspaceView ? 'workspace_workflow_queues' : 'scope_workflow_queues')} · ${observation.workflow_queues.length}`,
                  children: (
                    <WorkflowQueues queues={observation.workflow_queues} />
                  )
                }
              ]}
            />
          </>
        ))
      )}
      {diagnostics && observation && (
        <RuntimeDetails
          open
          observation={observation}
          snapshot={snapshot}
          fetchedAt={query.dataUpdatedAt}
          onClose={() => navigateEventState({ observation: null }, true)}
        />
      )}
    </div>
  );
}
