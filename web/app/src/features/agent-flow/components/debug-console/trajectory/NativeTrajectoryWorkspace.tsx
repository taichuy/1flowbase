import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  Button,
  Empty,
  Spin,
  Segmented,
  Select,
  TreeSelect
} from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import type {
  WorkflowTrajectoryIndexItem,
  WorkflowTrajectoryPage,
  WorkflowTrajectoryEvent,
  WorkflowTrajectoryCategory,
  ProviderTrajectoryStep,
  ProviderTrajectoryOptions
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { useTranslation } from 'react-i18next';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { purposeLabel } from './trajectory-presentation';
import {
  workflowEventLabel,
  workflowEventLane,
  workflowGroupKey,
  workflowNodeName
} from './workflow/presentation';
import { WorkflowEventDetail } from './workflow/WorkflowEventDetail';
import {
  containerRoundLabel,
  containerRoundOptions,
  findContainerRound,
  isWithinContainerRound
} from './workflow/container-rounds';
import { TrajectoryTimeline } from './TrajectoryTimeline';
import { type TrajectoryRange } from './timeline-model';
import { useTrajectoryWindow } from './virtual/use-trajectory-window';
import {
  TrajectoryVirtualList,
  type TrajectoryVirtualListRef
} from './virtual/TrajectoryVirtualList';
import './provider-trajectory.css';
import './workflow/workflow-trajectory.css';

type WorkflowSlot =
  | { key: string; event: WorkflowTrajectoryIndexItem }
  | { key: string; group: string; steps: WorkflowTrajectoryIndexItem[] };
const workflowSlotKey = (slot: WorkflowSlot) => slot.key;
const workflowSlotEventKey = (slot: WorkflowSlot) =>
  'event' in slot ? slot.event.event_id : undefined;
const workflowEventKey = (row: { event_id: string }) => row.event_id;

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
  const [roundFilter, setRoundFilter] = useState<string | undefined>();
  const { i18n } = useTranslation('agentFlow');
  const language = i18n.resolvedLanguage ?? i18n.language;
  const [groupCalls, setGroupCalls] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const list = useRef<TrajectoryVirtualListRef>(null);
  const filters = {
    category,
    focus_event_id: options?.focus_event_id,
    node_run_id: nodeFilter,
    request_id: options?.request_id,
    from: timeRange ? new Date(timeRange[0]).toISOString() : undefined,
    to: timeRange ? new Date(timeRange[1]).toISOString() : undefined
  };
  const {
    pages,
    overview: indexQuery,
    loaded,
    items,
    initialKey,
    onVisibleKeys,
    retry
  } = useTrajectoryWindow<
    WorkflowTrajectoryEvent,
    WorkflowTrajectoryIndexItem,
    WorkflowTrajectoryPage
  >({
    queryKey: ['workflow-trajectory', runId, filters],
    enabled: active && Boolean(loader.loadWorkflowTrajectory),
    loadPage: (focusId, indexOnly) =>
      loader.loadWorkflowTrajectory!(runId, undefined, {
        ...filters,
        index_only: indexOnly,
        focus_event_id: focusId ?? options?.focus_event_id
      }),
    getRowKey: workflowEventKey,
    getIndexKey: workflowEventKey
  });
  const [scopeMetadata, setScopeMetadata] = useState<{
    runId: string;
    page: Pick<WorkflowTrajectoryPage, 'nodes' | 'time_start' | 'time_end'>;
  }>();
  useEffect(() => {
    if (!indexQuery.data) return;
    const { nodes, time_start, time_end } = indexQuery.data;
    setScopeMetadata({ runId, page: { nodes, time_start, time_end } });
  }, [runId, indexQuery.data]);
  // The same backend run domain stays fixed while a new filter is pending.
  const overview =
    indexQuery.data ??
    (scopeMetadata?.runId === runId ? scopeMetadata.page : undefined);
  const allMatches = useMemo(
    () => indexQuery.data?.overview?.items ?? [],
    [indexQuery.data]
  );
  const containerAliases = useMemo(
    () =>
      new Map(
        (overview?.nodes ?? []).map((node) => [
          node.node_run_id,
          node.node_alias
        ])
      ),
    [overview?.nodes]
  );
  const roundOptions = useMemo(
    () =>
      containerRoundOptions(
        allMatches.map((step) => step.execution_identity),
        containerAliases,
        language
      ),
    [allMatches, containerAliases, language]
  );
  const matches = useMemo(() => {
    const path = findContainerRound(roundOptions, roundFilter);
    return path.length
      ? allMatches.filter((step) =>
          isWithinContainerRound(step.execution_identity, path)
        )
      : allMatches;
  }, [allMatches, roundOptions, roundFilter]);
  useEffect(() => {
    setRoundFilter(undefined);
  }, [runId]);
  const selectedStep = items.find(
    (step) =>
      step.event_id === selected || step.native_step?.event_id === selected
  );
  function resizeDetail(width: number) {
    const available = split.current?.clientWidth ?? 1000;
    setDetailWidth(Math.max(320, Math.min(720, available - 280, width)));
  }
  const groups = useMemo(() => {
    const result: Array<[string, WorkflowTrajectoryIndexItem[]]> = [];
    for (const step of matches) {
      const key = workflowGroupKey(step);
      const last = result.at(-1);
      if (last?.[0] === key) last[1].push(step);
      else result.push([key, [step]]);
    }
    return result;
  }, [matches]);
  function focusStep(step: WorkflowTrajectoryIndexItem) {
    setSelected(step.event_id);
    setCollapsed((current) => {
      const next = new Set(current);
      next.delete(workflowGroupKey(step));
      return next;
    });
    requestAnimationFrame(() =>
      list.current?.scrollTo({ key: step.event_id, align: 'auto' })
    );
  }
  const slots = useMemo<WorkflowSlot[]>(
    () =>
      groupCalls
        ? groups.flatMap(([group, steps]): WorkflowSlot[] => [
            { key: `group:${group}:${steps[0].event_id}`, group, steps },
            ...(!collapsed.has(group)
              ? steps.map((event) => ({ key: event.event_id, event }))
              : [])
          ])
        : matches.map((event) => ({ key: event.event_id, event })),
    [groupCalls, groups, collapsed, matches]
  );

  const timelinePoints = useMemo(
    () =>
      matches.map((step) => ({
        id: step.event_id,
        value: Date.parse(step.created_at),
        lane: workflowEventLane(step),
        label: `${workflowNodeName(step, language)} · ${workflowEventLabel(step, language)} · ${formatDateTime(step.created_at)}`
      })),
    [matches, language]
  );

  function renderStep(step: WorkflowTrajectoryEvent) {
    const isSelected = selected === step.event_id;
    return (
      <button
        key={step.event_id}
        type="button"
        className="provider-trajectory__row"
        data-lane={workflowEventLane(step)}
        data-selected={isSelected || undefined}
        aria-label={`${workflowEventLabel(step, language)} · ${workflowNodeName(step, language)} · ${step.node_run_id ?? step.flow_run_id}`}
        aria-pressed={isSelected}
        onClick={() => setSelected(step.event_id)}
      >
        <span className="provider-trajectory__marker" aria-hidden="true" />
        <span className="provider-trajectory__kind">
          {workflowEventLabel(step, language)}
        </span>
        <span className="provider-trajectory__preview">
          <strong>{workflowNodeName(step, language)}</strong>
          {step.execution_identity?.round_path.length ? (
            <span>
              {step.execution_identity.round_path
                .map((entry) =>
                  containerRoundLabel(entry, containerAliases, language)
                )
                .join(' / ')}
            </span>
          ) : null}
          {step.preview &&
          step.preview !== step.event_type &&
          step.preview !== workflowNodeName(step, language) ? (
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
        {roundOptions.length ? (
          <TreeSelect
            allowClear
            value={roundFilter}
            treeData={roundOptions}
            className="workflow-trajectory__node-filter"
            aria-label={i18nText(
              'agentFlow',
              'trajectory.container_round_filter'
            )}
            placeholder={i18nText(
              'agentFlow',
              'trajectory.all_container_rounds'
            )}
            onChange={setRoundFilter}
          />
        ) : null}
      </div>
      <TrajectoryTimeline
        laneLabels={[
          i18nText('agentFlow', 'trajectory.node_input_lane'),
          i18nText('agentFlow', 'trajectory.task_request_lane'),
          i18nText('agentFlow', 'auto.tools')
        ]}
        points={timelinePoints}
        range={timeRange}
        onChange={setTimeRange}
        selected={selected}
        extent={
          overview?.time_start && overview.time_end
            ? [Date.parse(overview.time_start), Date.parse(overview.time_end)]
            : undefined
        }
        onSelect={(id) => {
          const step = matches.find((item) => item.event_id === id);
          if (step) {
            focusStep(step);
          }
        }}
      />
      <div className="provider-trajectory__split" ref={split}>
        <section
          className="provider-trajectory__ledger"
          aria-label={i18nText('agentFlow', 'trajectory.steps')}
        >
          {pages.isLoading ? (
            <div className="provider-trajectory__loading" role="status">
              <Spin />
              <span>
                {i18nText('agentFlow', 'trajectory.loading_first_page')}
              </span>
            </div>
          ) : null}
          {pages.isError ? (
            <Alert
              type="error"
              showIcon
              title={i18nText('agentFlow', 'auto.loading_failed')}
              action={
                <Button onClick={retry}>
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
          {slots.length ? (
            <TrajectoryVirtualList
              ref={list}
              items={slots}
              getKey={workflowSlotKey}
              getEventKey={workflowSlotEventKey}
              initialKey={initialKey}
              resetKey={JSON.stringify([runId, filters])}
              onVisibleKeys={onVisibleKeys}
              active={active}
              renderItem={(slot) => {
                if ('event' in slot) {
                  const step = loaded.get(slot.event.event_id);
                  return step ? (
                    renderStep(step)
                  ) : (
                    <div
                      className="provider-trajectory__row trajectory-virtual-list__pending"
                      data-lane={workflowEventLane(slot.event)}
                      aria-busy="true"
                    >
                      <span
                        className="provider-trajectory__marker"
                        aria-hidden="true"
                      />
                      <span className="provider-trajectory__kind">
                        {workflowEventLabel(slot.event)}
                      </span>
                      <span className="provider-trajectory__preview">
                        <Spin size="small" />
                      </span>
                      <span className="provider-trajectory__row-time">
                        {formatDateTime(slot.event.created_at)}
                      </span>
                    </div>
                  );
                }
                const { group: key, steps } = slot;
                return (
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
                );
              }}
            />
          ) : null}
        </section>
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
                  list.current?.focusKey(selectedStep.event_id);
                  setSelected(null);
                }}
              />
            </div>
            <div
              className="workflow-trajectory__inspector-scroll"
              key={selectedStep.event_id}
            >
              <WorkflowEventDetail
                event={selectedStep}
                runId={runId}
                loader={loader}
                onClient={onClient}
              />
            </div>
          </aside>
        ) : null}
      </div>
      <footer className="provider-trajectory__footer">
        <span role="status">
          {pages.isError
            ? i18nText('agentFlow', 'auto.loading_failed')
            : pages.isPending
              ? i18nText('agentFlow', 'trajectory.loading_first_page')
              : pages.isFetching
                ? i18nText('agentFlow', 'trajectory.loading_pages')
                : i18nText('agentFlow', 'trajectory.loading_complete')}
        </span>
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
