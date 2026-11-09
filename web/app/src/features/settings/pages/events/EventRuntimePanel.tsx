import { ApiClientError, getEventRuntime } from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { Alert, Button, Spin, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useAuthStore } from '../../../../state/auth-store';
import { EventActivity } from './runtime/EventActivity';
import { QueueWorkbench } from './runtime/QueueWorkbench';
import { QueueStatistics } from './runtime/statistics/QueueStatistics';
import { ObservationStatistics } from './runtime/statistics/ObservationStatistics';
import { timeLabel, useRuntimeLabels } from './runtime/runtime-labels';
import './runtime/event-runtime.css';

const REFRESH_INTERVAL_MS = 5000;

export function EventRuntimePanel({ tab }: { tab: 'queues' | 'activity' }) {
  const { t } = useRuntimeLabels();
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const query = useQuery({
    queryKey: ['settings', 'events', 'runtime', workspaceId],
    queryFn: () => getEventRuntime(),
    gcTime: 0,
    retry: false,
    refetchInterval: (query) =>
      query.state.error instanceof ApiClientError &&
      [401, 403].includes(query.state.error.status)
        ? false
        : REFRESH_INTERVAL_MS,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: true,
    refetchOnReconnect: true
  });
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 15000);
    return () => window.clearInterval(timer);
  }, []);
  const snapshot = query.data;
  const denied =
    query.error instanceof ApiClientError &&
    [401, 403].includes(query.error.status);
  // A rejected authorization response must not continue displaying an older privileged snapshot.
  const observations = denied ? [] : (snapshot?.source_views ?? []);
  const stale = observations.some(
    (observation) =>
      Math.max(now, Date.now()) - query.dataUpdatedAt >=
      observation.observation_ttl_ms
  );
  // The workspace queues already include its source-bound subscriptions.
  const queues = [
    ...(snapshot?.queues ?? []),
    ...observations.flatMap((observation) =>
      observation.scope.kind === 'workspace' &&
      observation.scope.workspace_id === snapshot?.workspace_id
        ? []
        : observation.workflow_queues
    )
  ];
  const refresh = () => void query.refetch({ cancelRefetch: false });
  const toolbarActions = (
    <>
      <Button
        data-event-runtime-refresh
        aria-label={t('refresh')}
        onClick={refresh}
        loading={query.isFetching}
        icon={<ReloadOutlined />}
      >
        {t('refresh')}
      </Button>
    </>
  );
  return (
    <div className="event-runtime" aria-busy={query.isFetching}>
      {snapshot &&
        !denied &&
        observations.length > 0 &&
        (tab === 'queues' ? (
          <QueueStatistics
            key={`${workspaceId}:${snapshot.instance_id}`}
            queues={queues}
            capturedAt={query.dataUpdatedAt}
          />
        ) : (
          <ObservationStatistics
            key={`${workspaceId}:${snapshot.instance_id}`}
            views={observations}
            capturedAt={query.dataUpdatedAt}
            admissionRejected={snapshot.admission_rejected}
          />
        ))}
      {query.isPaused && (
        <Typography.Text
          type="secondary"
          className="event-runtime-freshness"
          role="status"
        >
          {t('runtime_waiting_for_network')}
        </Typography.Text>
      )}
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
      {!denied && snapshot?.workflow_refresh_error && (
        <Alert type="warning" showIcon title={t('workflow_refresh_error')} />
      )}
      {observations.some((view) => !view.configured) && (
        <Alert type="warning" showIcon title={t('unconfigured')} />
      )}
      {snapshot && !denied && !observations.length && (
        <Alert type="warning" showIcon title={t('runtime_scope_unavailable')} />
      )}
      {tab === 'activity' &&
        observations.some(
          ({ observation_loss: loss }) =>
            loss.accounting_available &&
            (loss.evicted > 0 || loss.oversize_dropped > 0)
        ) && (
          <Alert type="warning" showIcon title={t('runtime_records_limited')} />
        )}
      {query.isPending ? (
        <Spin description={t('runtime_loading')}>
          <div className="event-runtime-loading" />
        </Spin>
      ) : (
        snapshot &&
        !denied &&
        (tab === 'activity' ? (
          <EventActivity
            activity={observations.flatMap((view) => view.activity)}
            sources={observations.flatMap((view) => view.sources)}
            toolbarActions={toolbarActions}
          />
        ) : (
          <QueueWorkbench
            queues={queues}
            workflows={snapshot.workflow_subscriptions}
            toolbarActions={toolbarActions}
          />
        ))
      )}
    </div>
  );
}
