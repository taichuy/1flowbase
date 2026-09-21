import { useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Descriptions, Empty, Spin, Tabs } from 'antd';
import type { ClientTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { JsonPreviewBlock } from '../../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { DebugMarkdownContent } from '../../conversation/DebugMarkdownContent';
import { sectionLabel } from './presentation';

/** Renders backend-selected sections; never reconstructs protocol facts from Native. */
export function ClientTrajectoryDetail({
  step,
  loader,
  nodeRunId,
  onRelated
}: {
  step: ClientTrajectoryStep;
  loader: ConversationLogTraceLoader;
  nodeRunId?: string;
  onRelated: (id: string) => void;
}) {
  const [section, setSection] = useState(
    step.available_sections[0] ?? 'overview'
  );
  const pages = useInfiniteQuery({
    queryKey: [
      'client-trajectory-section',
      step.flow_run_id,
      nodeRunId,
      step.id,
      section
    ],
    enabled: Boolean(
      loader.loadClientTrajectorySection &&
      step.available_sections.includes(section)
    ),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadClientTrajectorySection!(
        step.flow_run_id,
        step.id,
        section,
        nodeRunId,
        pageParam
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  return (
    <div className="provider-trajectory__detail client-trajectory__detail">
      <Descriptions
        size="small"
        column={1}
        items={[
          {
            key: 'origin',
            label: i18nText('agentFlow', 'clientTrajectory.direction'),
            children:
              step.origin === 'submitted'
                ? i18nText('agentFlow', 'clientTrajectory.submitted')
                : i18nText('agentFlow', 'clientTrajectory.emitted')
          },
          {
            key: 'protocol',
            label: i18nText('agentFlow', 'clientTrajectory.protocol'),
            children: `${step.protocol} · ${step.transport}`
          },
          ...(step.call_id
            ? [{ key: 'call_id', label: 'call_id', children: step.call_id }]
            : []),
          ...(step.response_id
            ? [
                {
                  key: 'response_id',
                  label: 'response_id',
                  children: step.response_id
                }
              ]
            : [])
        ]}
      />
      {step.related_step_id ? (
        <Button
          size="small"
          type="link"
          onClick={() => onRelated(step.related_step_id!)}
        >
          {i18nText('agentFlow', 'clientTrajectory.related')}
        </Button>
      ) : null}
      <Tabs
        size="small"
        activeKey={section}
        onChange={setSection}
        items={step.available_sections.map((key) => ({
          key,
          label: sectionLabel(key)
        }))}
      />
      {section === 'raw' ? (
        <p className="client-trajectory__note">
          {i18nText('agentFlow', 'clientTrajectory.raw_scope')}
        </p>
      ) : null}
      {section === 'timing' ? (
        <p className="client-trajectory__note">
          {i18nText('agentFlow', 'clientTrajectory.timing_scope')}
        </p>
      ) : null}
      {pages.isLoading ? <Spin /> : null}
      {pages.isError ? (
        <Alert
          type="error"
          title={i18nText('agentFlow', 'auto.loading_failed')}
          action={
            <Button onClick={() => void pages.refetch()}>
              {i18nText('agentFlow', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
      {pages.data?.pages.map((page) =>
        page.items.map((item) => (
          <SectionValue
            key={item.sequence}
            value={item.value}
            section={section}
          />
        ))
      )}
      {pages.data && !pages.data.pages.some((page) => page.items.length) ? (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : null}
      {pages.hasNextPage ? (
        <Button
          loading={pages.isFetchingNextPage}
          onClick={() => void pages.fetchNextPage()}
        >
          {i18nText('agentFlow', 'trajectory.more')}
        </Button>
      ) : null}
    </div>
  );
}
function SectionValue({ value, section }: { value: unknown; section: string }) {
  const title = sectionLabel(section);
  if (
    section === 'raw' &&
    value &&
    typeof value === 'object' &&
    'body' in value &&
    typeof value.body === 'string'
  ) {
    return (
      <JsonPreviewBlock
        title={title}
        value={value}
        rawText={value.body}
        collapsible={false}
        height="420px"
      />
    );
  }
  if (typeof value === 'string') {
    return section === 'parameters' ? (
      <pre className="client-trajectory__text">{value}</pre>
    ) : (
      <DebugMarkdownContent content={value} />
    );
  }
  // Typed Responses content parts stay in their original order; non-text parts retain their exact value.
  if (section === 'result' && Array.isArray(value)) {
    return (
      <>
        {value.map((part: unknown, index) =>
          part &&
          typeof part === 'object' &&
          'text' in part &&
          typeof part.text === 'string' ? (
            <DebugMarkdownContent key={index} content={part.text} />
          ) : (
            <JsonPreviewBlock
              key={index}
              title={title}
              value={part}
              collapsible={false}
            />
          )
        )}
      </>
    );
  }
  if (
    (section === 'overview' || section === 'timing' || section === 'usage') &&
    value &&
    typeof value === 'object' &&
    !Array.isArray(value)
  ) {
    return (
      <Descriptions
        className="client-trajectory__fields"
        size="small"
        column={1}
        items={Object.entries(value).map(([key, field]) => ({
          key,
          label: key,
          children:
            field !== null && typeof field === 'object' ? (
              <JsonPreviewBlock title={key} value={field} height="180px" />
            ) : (
              <span className="client-trajectory__text">
                {String(field ?? '—')}
              </span>
            )
        }))}
      />
    );
  }
  return (
    <JsonPreviewBlock
      title={title}
      value={value}
      collapsible={false}
      height="360px"
    />
  );
}
