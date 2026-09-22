import { useEffect, useMemo, useRef, useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Segmented, Select } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import type {
  WorkflowTrajectoryPage,
  WorkflowTrajectoryEvent,
  WorkflowTrajectoryCategory,
  ProviderTrajectoryStep,
  ProviderTrajectoryOptions
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { TrajectoryStepDetail } from './TrajectoryStepDetail';
import { purposeLabel } from './trajectory-presentation';
import {
  workflowEventLabel,
  workflowEventLane,
  workflowGroupKey,
  workflowNodeName
} from './workflow/presentation';
import { WorkflowEventDetail } from './workflow/WorkflowEventDetail';
import { TrajectoryTimeline, type TrajectoryRange } from './TrajectoryTimeline';
import { useProgressiveTrajectory } from './use-progressive-trajectory';
import './provider-trajectory.css';
import './workflow/workflow-trajectory.css';

export function NativeTrajectoryWorkspace({
  runId,
  nodeRunId,
  loader,
  options,
  active = true,
  onClient
}: {
  active?: boolean;
  runId: string;
  nodeRunId?: string;
  loader: ConversationLogTraceLoader;
  options?: ProviderTrajectoryOptions;
  onClient?: (
    link: NonNullable<ProviderTrajectoryStep['links']>[number]
  ) => void;
}) {
  const [selected, setSelected] = useState<string | null>(
    options?.focus_event_id ?? null
  );
  const [detailWidth, setDetailWidth] = useState<number | null>(null);
  const split = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const [timeRange, setTimeRange] = useState<TrajectoryRange>(null);
  const [category, setCategory] = useState<WorkflowTrajectoryCategory>('all');
  const [nodeFilter, setNodeFilter] = useState<string | undefined>(nodeRunId);
  const [groupCalls, setGroupCalls] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const filters = {
    category,
    node_run_id: nodeFilter,
    request_id: options?.request_id,
    from: timeRange ? new Date(timeRange[0]).toISOString() : undefined,
    to: timeRange ? new Date(timeRange[1]).toISOString() : undefined
  };
  const pages = useInfiniteQuery({
    queryKey: ['workflow-trajectory', runId, filters],
    enabled: active && Boolean(loader.loadWorkflowTrajectory),
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadWorkflowTrajectory!(runId, pageParam, filters),
    getNextPageParam: (page, all, cursor) =>
      page.next_cursor &&
      page.next_cursor !== cursor &&
      !all
        .slice(0, -1)
        .some((previous) => previous.next_cursor === page.next_cursor)
        ? page.next_cursor
        : undefined,
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(active, pages);
  const items = useMemo(
    () => pages.data?.pages.flatMap((page) => page.items) ?? [],
    [pages.data]
  );
  const [scopeOverview, setScopeOverview] = useState<WorkflowTrajectoryPage>();
  const overview = pages.data?.pages[0] ?? scopeOverview;
  useEffect(() => {
    if (pages.data?.pages[0]) setScopeOverview(pages.data.pages[0]);
  }, [pages.data]);
  const selectedStep = items.find(
    (step) =>
      step.event_id === selected || step.native_step?.event_id === selected
  );
  function resizeDetail(width: number) {
    const available = split.current?.clientWidth ?? 1000;
    setDetailWidth(Math.max(320, Math.min(720, available - 280, width)));
  }
  const matches = items;
  const groups = useMemo(() => {
    const result: Array<[string, WorkflowTrajectoryEvent[]]> = [];
    for (const step of matches) {
      const key = workflowGroupKey(step);
      const last = result.at(-1);
      if (last?.[0] === key) last[1].push(step);
      else result.push([key, [step]]);
    }
    return result;
  }, [matches]);
  function focusStep(step: WorkflowTrajectoryEvent) {
    setSelected(step.event_id);
    setCollapsed((current) => {
      const next = new Set(current);
      next.delete(workflowGroupKey(step));
      return next;
    });
    requestAnimationFrame(() =>
      rows.current
        .get(step.event_id)
        ?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' })
    );
  }
  function renderStep(step: WorkflowTrajectoryEvent) {
    const isSelected = selected === step.event_id;
    return (
      <button
        key={step.event_id}
        ref={(element) => {
          if (element) rows.current.set(step.event_id, element);
          else rows.current.delete(step.event_id);
        }}
        type="button"
        className="provider-trajectory__row"
        data-lane={workflowEventLane(step)}
        data-selected={isSelected || undefined}
        aria-label={`${workflowEventLabel(step)} · ${workflowNodeName(step)} · ${step.node_run_id ?? step.flow_run_id}`}
        aria-pressed={isSelected}
        onClick={() => setSelected(step.event_id)}
      >
        <span className="provider-trajectory__marker" aria-hidden="true" />
        <span className="provider-trajectory__kind">
          {workflowEventLabel(step)}
        </span>
        <span className="provider-trajectory__preview">
          <strong>{workflowNodeName(step)}</strong>
          {step.preview &&
          step.preview !== step.event_type &&
          step.preview !== workflowNodeName(step) ? (
            <span>{step.preview}</span>
          ) : null}
        </span>
        <span
          className="provider-trajectory__row-time"
          title={formatDateTime(step.created_at)}
        >
          {formatDateTime(step.created_at)}
        </span>
      </button>
    );
  }
  return (
    <div className="provider-trajectory workflow-trajectory">
      <Segmented
        className="workflow-trajectory__categories"
        aria-label={i18nText('agentFlow', 'trajectory.activity_category')}
        value={category}
        onChange={(value) => setCategory(value as WorkflowTrajectoryCategory)}
        options={[
          {
            value: 'all',
            label: i18nText('agentFlow', 'trajectory.all_events')
          },
          {
            value: 'nodes',
            label: i18nText('agentFlow', 'trajectory.node_events')
          },
          {
            value: 'requests',
            label: i18nText('agentFlow', 'client_trajectory.request')
          },
          { value: 'tools', label: i18nText('agentFlow', 'auto.tools') },
          {
            value: 'rounds',
            label: i18nText('agentFlow', 'trajectory.round_activities')
          },
          {
            value: 'agents',
            label: i18nText('agentFlow', 'trajectory.agent_activities')
          }
        ]}
      />
      <div
        className="provider-trajectory__toolbar"
        role="toolbar"
        aria-label={i18nText('agentFlow', 'trajectory.overview')}
      >
        <div className="provider-trajectory__controls">
          <Button
            size="small"
            type="text"
            icon={<ApartmentOutlined />}
            aria-label={i18nText('agentFlow', 'trajectory.group_calls')}
            aria-pressed={groupCalls}
            onClick={() => setGroupCalls(!groupCalls)}
          >
            {i18nText('agentFlow', 'trajectory.group_calls')}
          </Button>
        </div>
        <Select
          allowClear={!nodeRunId}
          value={nodeFilter}
          disabled={Boolean(nodeRunId)}
          className="workflow-trajectory__node-filter"
          aria-label={i18nText('agentFlow', 'trajectory.node_filter')}
          placeholder={i18nText('agentFlow', 'trajectory.all_nodes')}
          options={(overview?.nodes ?? []).map((node) => ({
            value: node.node_run_id,
            label: `${workflowNodeName(node)} · ${node.node_id}`
          }))}
          onChange={setNodeFilter}
        />
      </div>
      <TrajectoryTimeline
        laneLabels={[
          i18nText('agentFlow', 'trajectory.node_input_lane'),
          i18nText('agentFlow', 'trajectory.task_request_lane'),
          i18nText('agentFlow', 'auto.tools')
        ]}
        points={items.map((step) => ({
          id: step.event_id,
          value: Date.parse(step.created_at),
          lane: workflowEventLane(step),
          label: `${workflowNodeName(step)} · ${workflowEventLabel(step)} · ${formatDateTime(step.created_at)}`
        }))}
        range={timeRange}
        onChange={setTimeRange}
        selected={selected}
        extent={
          overview?.time_start && overview.time_end
            ? [Date.parse(overview.time_start), Date.parse(overview.time_end)]
            : undefined
        }
        onSelect={(id) => {
          const step = items.find((item) => item.event_id === id);
          if (step) {
            focusStep(step);
          }
        }}
      />
      <div className="provider-trajectory__split" ref={split}>
        <div
          className="provider-trajectory__ledger"
          role="region"
          aria-label={i18nText('agentFlow', 'trajectory.steps')}
        >
          {pages.isLoading ? (
            <div className="provider-trajectory__loading">
              <Spin />
            </div>
          ) : null}
          {pages.isError ? (
            <Alert
              type="error"
              showIcon
              title={i18nText('agentFlow', 'auto.loading_failed')}
              action={
                <Button
                  onClick={() =>
                    void (pages.isFetchNextPageError
                      ? pages.fetchNextPage()
                      : pages.refetch())
                  }
                >
                  {i18nText('agentFlow', 'auto.retry')}
                </Button>
              }
            />
          ) : null}
          {!pages.isLoading && !pages.isError && !matches.length ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                items.length
                  ? i18nText('agentFlow', 'trajectory.no_matches')
                  : options?.request_id
                    ? i18nText('agentFlow', 'trajectory.no_internal_calls')
                    : i18nText('agentFlow', 'trajectory.not_recorded_detail')
              }
            />
          ) : null}
          {groupCalls
            ? groups.map(([key, steps]) => (
                <section key={`${key}:${steps[0].event_id}`}>
                  <button
                    type="button"
                    className="provider-trajectory__group"
                    aria-expanded={!collapsed.has(key)}
                    onClick={() =>
                      setCollapsed((current) => {
                        const next = new Set(current);
                        if (next.has(key)) next.delete(key);
                        else next.add(key);
                        return next;
                      })
                    }
                  >
                    {collapsed.has(key) ? <RightOutlined /> : <DownOutlined />}
                    <strong>{workflowNodeName(steps[0])}</strong>
                    {steps[0].native_step ? (
                      <span>
                        {purposeLabel(steps[0].native_step.metadata.purpose)}
                      </span>
                    ) : null}
                    <span className="provider-trajectory__group-id">
                      {steps[0].native_step?.metadata.invocation_id ??
                        steps[0].node_id ??
                        steps[0].task_run_id}
                    </span>
                    {steps[0].parent_task_run_id ? (
                      <span>
                        {i18nText('agentFlow', 'trajectory.parent_task')}:{' '}
                        {steps[0].parent_task_run_id}
                      </span>
                    ) : null}
                    <span>
                      {i18nText('agentFlow', 'trajectory.loaded_steps', {
                        count: steps.length
                      })}
                    </span>
                  </button>
                  {!collapsed.has(key) ? steps.map(renderStep) : null}
                </section>
              ))
            : matches.map(renderStep)}
          {pages.hasNextPage && !pages.isError ? (
            <div className="provider-trajectory__more" role="status">
              <Spin size="small" />{' '}
              {i18nText('agentFlow', 'trajectory.loading_pages')}
            </div>
          ) : null}
        </div>
        {selectedStep ? (
          <aside
            className="provider-trajectory__inspector"
            style={detailWidth === null ? undefined : { width: detailWidth }}
            aria-label={i18nText('agentFlow', 'trajectory.inspector')}
            data-lane={workflowEventLane(selectedStep)}
          >
            <div
              className="provider-trajectory__resize"
              role="separator"
              tabIndex={0}
              aria-orientation="vertical"
              aria-label={i18nText('agentFlow', 'trajectory.inspector')}
              aria-valuemin={320}
              aria-valuemax={720}
              aria-valuenow={detailWidth ?? 440}
              onPointerDown={(event) => {
                drag.current = {
                  x: event.clientX,
                  width:
                    event.currentTarget.parentElement!.getBoundingClientRect()
                      .width
                };
                event.currentTarget.setPointerCapture(event.pointerId);
              }}
              onPointerMove={(event) => {
                if (drag.current)
                  resizeDetail(
                    drag.current.width + drag.current.x - event.clientX
                  );
              }}
              onPointerUp={() => {
                drag.current = null;
              }}
              onPointerCancel={() => {
                drag.current = null;
              }}
              onKeyDown={(event) => {
                if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
                  const width =
                    event.currentTarget.parentElement!.getBoundingClientRect()
                      .width;
                  resizeDetail(width + (event.key === 'ArrowLeft' ? 16 : -16));
                  event.preventDefault();
                }
              }}
            />
            <div className="provider-trajectory__inspector-header">
              <span className="provider-trajectory__kind">
                {workflowEventLabel(selectedStep)}
              </span>
              <span className="provider-trajectory__row-time">
                {formatDateTime(selectedStep.created_at)}
              </span>
              <Button
                size="small"
                type="text"
                icon={<CloseOutlined />}
                aria-label={i18nText('agentFlow', 'auto.close', {
                  value1: i18nText('agentFlow', 'trajectory.inspector')
                })}
                onClick={() => {
                  const row = rows.current.get(selectedStep.event_id);
                  setSelected(null);
                  row?.focus();
                }}
              />
            </div>
            <div
              className="workflow-trajectory__inspector-scroll"
              key={selectedStep.event_id}
            >
              <WorkflowEventDetail
                key={selectedStep.event_id}
                event={selectedStep}
                runId={runId}
                loader={loader}
              >
                {selectedStep.native_step ? (
                  <TrajectoryStepDetail
                    step={selectedStep.native_step}
                    loader={loader}
                    onClient={onClient}
                  />
                ) : null}
              </WorkflowEventDetail>
            </div>
          </aside>
        ) : null}
      </div>
      <footer className="provider-trajectory__footer">
        <span>{i18nText('agentFlow', 'trajectory.chronology_note')}</span>
        <span>
          {i18nText('agentFlow', 'trajectory.loaded_steps', {
            count: items.length
          })}
        </span>
        <span>
          {nodeRunId
            ? i18nText('agentFlow', 'trajectory.node_scope')
            : i18nText('agentFlow', 'trajectory.run_scope')}
        </span>
      </footer>
    </div>
  );
}
