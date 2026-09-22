import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Tabs } from 'antd';
import type {
  WorkflowTrajectoryEvent,
  ProviderTrajectoryStep
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { JsonPreviewBlock } from '../../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { eventSectionTab } from '../trajectory-presentation';
import { TrajectoryStepDetail } from '../TrajectoryStepDetail';
import { workflowNodeName, workflowSectionLabel } from './presentation';
export function WorkflowEventDetail({
  event,
  runId,
  loader,
  onClient
}: {
  event: WorkflowTrajectoryEvent;
  runId: string;
  loader: ConversationLogTraceLoader;
  onClient?: (link: ProviderTrajectoryStep['links'][number]) => void;
}) {
  const [tab, setTab] = useState(() =>
    event.event_type === 'node_finished' ||
    ['model_reply', 'tool_result'].includes(
      String(event.native_step?.metadata.kind)
    )
      ? 'output'
      : event.event_type === 'node_started' ||
          event.native_step?.metadata.kind === 'model_call'
        ? 'input'
        : 'process'
  );
  const native = event.native_step;
  const trigger = native?.links.find((link) => link.relation === 'trigger');
  const body = useQuery({
    queryKey: ['workflow-trajectory-body', runId, event.event_id],
    enabled:
      !native &&
      tab !== 'metadata' &&
      Boolean(loader.loadWorkflowTrajectoryBody),
    queryFn: () => loader.loadWorkflowTrajectoryBody!(runId, event.event_id),
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  const workflowBody = (
    <section className="provider-trajectory__detail">
      {body.isLoading ? <Spin /> : null}
      {body.isError ? (
        <Alert
          type="error"
          title={i18nText('agentFlow', 'auto.loading_failed')}
          action={
            <Button onClick={() => void body.refetch()}>
              {i18nText('agentFlow', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
      {body.data ? (
        tab === 'raw' ? (
          <JsonPreviewBlock
            title={i18nText('agentFlow', 'trajectory.semantic_raw')}
            value={body.data}
            collapsible={false}
          />
        ) : (
          body.data.sections
            .filter((section) => eventSectionTab(section.kind) === tab)
            .map((section, index) => (
              <JsonPreviewBlock
                key={`${section.kind}:${index}`}
                title={workflowSectionLabel(section.kind)}
                value={section.value}
                collapsible={false}
              />
            ))
        )
      ) : null}
      {body.isSuccess &&
      tab !== 'raw' &&
      !body.data.sections.some(
        (section) => eventSectionTab(section.kind) === tab
      ) ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'trajectory.no_evidence')}
        />
      ) : null}
    </section>
  );
  return (
    <>
      <div className="workflow-trajectory__event-summary">
        <strong>{workflowNodeName(event)}</strong>
        {event.status ? <span>{event.status}</span> : null}
        {trigger && onClient ? (
          <Button
            type="link"
            size="small"
            title={trigger.request_id}
            onClick={() => onClient(trigger)}
          >
            {i18nText('agentFlow', 'trajectory.trigger_request')}
          </Button>
        ) : null}
      </div>
      <Tabs
        className="workflow-trajectory__detail-tabs"
        size="small"
        tabBarGutter={16}
        activeKey={tab}
        onChange={setTab}
        destroyOnHidden
        items={[
          ...(
            [
              { key: 'input', label: i18nText('agentFlow', 'auto.input') },
              {
                key: 'process',
                label: i18nText('agentFlow', 'auto.data_processing')
              },
              {
                key: 'output',
                label: i18nText('agentFlow', 'auto.outputs')
              }
            ] as const
          ).map((item) => ({
            ...item,
            children: native ? (
              <TrajectoryStepDetail
                step={native}
                loader={loader}
                view={item.key}
              />
            ) : (
              workflowBody
            )
          })),
          {
            key: 'metadata',
            label: i18nText('agentFlow', 'auto.metadata'),
            children: (
              <>
                <div className="provider-trajectory__detail workflow-trajectory__identity">
                  <small>
                    {event.node_id} · {event.node_run_id}
                  </small>
                  {event.task_run_id ? (
                    <span>
                      {i18nText('agentFlow', 'trajectory.task')}:{' '}
                      {event.task_run_id}
                    </span>
                  ) : null}
                  {event.parent_task_run_id ? (
                    <span>
                      {i18nText('agentFlow', 'trajectory.parent_task')}:{' '}
                      {event.parent_task_run_id}
                    </span>
                  ) : null}
                </div>
                {native ? (
                  <TrajectoryStepDetail
                    step={native}
                    loader={loader}
                    view="metadata"
                    onClient={onClient}
                  />
                ) : null}
              </>
            )
          },
          {
            key: 'raw',
            label: i18nText('agentFlow', 'trajectory.semantic_raw'),
            children: native ? (
              <TrajectoryStepDetail step={native} loader={loader} view="raw" />
            ) : (
              workflowBody
            )
          }
        ]}
      />
    </>
  );
}
