import { useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Tabs } from 'antd';
import type {
  ProviderTrajectoryStep,
  ProviderTrajectoryView
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { nativeSectionLabel, purposeLabel } from './trajectory-presentation';
import { JsonPreviewBlock } from '../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { formatDurationScaled } from '../conversation/metrics-formatter';
import { formatDateTime } from '../../../../../shared/i18n/format';

export function TrajectoryStepDetail({
  step,
  loader,
  onClient
}: {
  step: ProviderTrajectoryStep;
  loader: ConversationLogTraceLoader;
  onClient?: (
    link: NonNullable<ProviderTrajectoryStep['links']>[number]
  ) => void;
}) {
  const [rawSemantic, setRawSemantic] = useState(false);
  const [view, setView] = useState<ProviderTrajectoryView>('semantic');
  const { flow_run_id, node_run_id } = step.metadata;
  const body = useInfiniteQuery({
    queryKey: [
      'provider-trajectory-body',
      flow_run_id,
      node_run_id,
      step.event_id,
      view
    ],
    enabled: Boolean(loader.loadTrajectoryBody),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadTrajectoryBody!(
        flow_run_id,
        node_run_id,
        step.event_id,
        pageParam,
        view
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    // Layout switches must reuse the selected evidence, like other log details.
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  return (
    <section className="provider-trajectory__detail">
      <div className="provider-trajectory__detail-meta">
        <span>
          {step.metadata.source === 'ai_native'
            ? i18nText('agentFlow', 'trajectory.native_source')
            : i18nText('agentFlow', 'trajectory.supplier_source')}
        </span>
        <time>{formatDateTime(step.created_at)}</time>
        <span>{step.metadata.node_id || node_run_id}</span>
        <span title={step.metadata.invocation_id}>
          {i18nText('agentFlow', 'trajectory.invocation')}:{' '}
          {step.metadata.invocation_id}
        </span>
      </div>
      <div className="provider-trajectory__detail-meta">
        <span>{purposeLabel(step.metadata.purpose)}</span>
        {step.metadata.duration_ms != null ? (
          <span title={`${step.metadata.duration_ms} ms`}>
            {i18nText('agentFlow', 'trajectory.duration')}:{' '}
            {formatDurationScaled(step.metadata.duration_ms)}
          </span>
        ) : null}
        {step.metadata.run_mode ? <span>{step.metadata.run_mode}</span> : null}
        {step.links.length ? (
          step.links.map((link, index) => (
            <Button
              key={`${link.relation}:${link.request_id}:${index}`}
              type="link"
              size="small"
              onClick={() => onClient?.(link)}
              disabled={!onClient}
            >
              {link.relation === 'trigger'
                ? i18nText('agentFlow', 'trajectory.trigger_request')
                : i18nText('agentFlow', 'trajectory.context_request')}{' '}
              · {link.request_id}
            </Button>
          ))
        ) : (
          <span>{i18nText('agentFlow', 'trajectory.source_not_recorded')}</span>
        )}
      </div>
      <Tabs
        size="small"
        activeKey={rawSemantic ? 'semantic_raw' : view}
        onChange={(key) => {
          setRawSemantic(key === 'semantic_raw');
          setView(key === 'protocol' ? 'protocol' : 'semantic');
        }}
        items={[
          {
            key: 'semantic',
            label: i18nText('agentFlow', 'trajectory.step_detail')
          },
          {
            key: 'semantic_raw',
            label: i18nText('agentFlow', 'trajectory.semantic_raw')
          },
          {
            key: 'protocol',
            label: i18nText('agentFlow', 'trajectory.raw_evidence')
          }
        ]}
      />
      {view === 'protocol' &&
      body.data?.pages[0]?.evidence_scope === 'invocation' ? (
        <Alert
          type="info"
          showIcon
          title={i18nText('agentFlow', 'trajectory.invocation_evidence')}
        />
      ) : null}
      {body.isLoading ? <Spin /> : null}
      {body.isError ? (
        <Alert
          type="error"
          showIcon
          title={i18nText('agentFlow', 'auto.loading_failed')}
          action={
            <Button onClick={() => void body.refetch()}>
              {i18nText('agentFlow', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
      {body.isSuccess &&
      !body.data.pages.some((page) =>
        view === 'semantic' && !rawSemantic
          ? page.sections.length
          : page.items.length
      ) ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'trajectory.no_evidence')}
        />
      ) : null}
      {view === 'semantic' && !rawSemantic
        ? body.data?.pages
            .flatMap((page) => page.sections)
            .map((section, index) => (
              <section
                key={`${section.kind}:${index}`}
                aria-label={nativeSectionLabel(section.kind)}
              >
                <h4>{nativeSectionLabel(section.kind)}</h4>
                {typeof section.value === 'string' ? (
                  <pre className="provider-trajectory__body">
                    {section.value}
                  </pre>
                ) : (
                  <JsonPreviewBlock
                    title={nativeSectionLabel(section.kind)}
                    value={section.value}
                    collapsible={false}
                  />
                )}
              </section>
            ))
        : body.data?.pages
            .flatMap((page) => page.items)
            .map((evidence) => (
              <pre
                key={evidence.event_id}
                className="provider-trajectory__body"
              >
                {evidence.body}
              </pre>
            ))}
      {body.hasNextPage ? (
        <Button
          size="small"
          loading={body.isFetchingNextPage}
          onClick={() => void body.fetchNextPage()}
        >
          {i18nText('agentFlow', 'trajectory.more_evidence')}
        </Button>
      ) : null}
    </section>
  );
}
