import { useMemo, useRef, useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Input, Spin, Tooltip } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import ClockCircleOutlined from '@ant-design/icons/es/icons/ClockCircleOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import UnorderedListOutlined from '@ant-design/icons/es/icons/UnorderedListOutlined';
import type {
  ProviderTrajectoryStep,
  ProviderTrajectoryOptions
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { TrajectoryStepDetail } from './TrajectoryStepDetail';
import {
  integrityLabel,
  purposeLabel,
  invocationKey,
  stepLabel,
  stepLane
} from './trajectory-presentation';
import {
  TrajectoryTimeline,
  inTrajectoryRange,
  type TrajectoryRange
} from './TrajectoryTimeline';
import { useProgressiveTrajectory } from './use-progressive-trajectory';
import './provider-trajectory.css';

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
  const [search, setSearch] = useState('');
  const [timeScale, setTimeScale] = useState(true);
  const [groupCalls, setGroupCalls] = useState(true);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const pages = useInfiniteQuery({
    queryKey: ['provider-trajectory', runId, nodeRunId ?? 'run', options],
    enabled:
      active &&
      Boolean(nodeRunId ? loader.loadTrajectory : loader.loadRunTrajectory),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      nodeRunId
        ? loader.loadTrajectory!(runId, nodeRunId, pageParam, options)
        : loader.loadRunTrajectory!(runId, pageParam, options),
    getNextPageParam: (page, _pages, cursor) =>
      page.next_cursor != null &&
      (cursor === undefined || page.next_cursor > cursor)
        ? page.next_cursor
        : undefined,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(active, pages);
  const items = useMemo(
    () => pages.data?.pages.flatMap((page) => page.items) ?? [],
    [pages.data]
  );
  const overview = pages.data?.pages[0];
  const selectedStep = items.find((step) => step.event_id === selected);
  function resizeDetail(width: number) {
    const available = split.current?.clientWidth ?? 1000;
    setDetailWidth(Math.max(320, Math.min(720, available - 280, width)));
  }
  const query = search.trim().toLocaleLowerCase();
  const matches = items.filter(
    (step, index) =>
      inTrajectoryRange(
        timeScale ? Date.parse(step.created_at) : index,
        timeRange
      ) &&
      (!query ||
        [
          stepLabel(step),
          step.metadata.preview,
          step.metadata.node_id,
          step.metadata.tool_call_id,
          step.metadata.invocation_id
        ]
          .filter(Boolean)
          .join(' ')
          .toLocaleLowerCase()
          .includes(query))
  );
  const groups = useMemo(() => {
    const result = new Map<string, ProviderTrajectoryStep[]>();
    for (const step of matches) {
      const key = invocationKey(step);
      const group = result.get(key) ?? [];
      group.push(step);
      result.set(key, group);
    }
    return [...result.entries()];
  }, [matches]);
  function focusStep(step: ProviderTrajectoryStep) {
    setSelected(step.event_id);
    setCollapsed((current) => {
      const next = new Set(current);
      next.delete(invocationKey(step));
      return next;
    });
    requestAnimationFrame(() =>
      rows.current
        .get(step.event_id)
        ?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' })
    );
  }
  function renderStep(step: ProviderTrajectoryStep) {
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
        data-lane={stepLane(step)}
        data-selected={isSelected || undefined}
        aria-label={stepLabel(step)}
        aria-pressed={isSelected}
        onClick={() => setSelected(step.event_id)}
      >
        <span className="provider-trajectory__marker" aria-hidden="true" />
        <span className="provider-trajectory__kind">{stepLabel(step)}</span>
        <span className="provider-trajectory__preview">
          {step.metadata.preview ||
            step.metadata.tool_call_id ||
            step.metadata.node_id ||
            '—'}
        </span>
        <span
          className="provider-trajectory__row-time"
          title={formatDateTime(step.created_at)}
        >
          #{step.event_sequence}
        </span>
      </button>
    );
  }
  return (
    <div className="provider-trajectory">
      <div
        className="provider-trajectory__toolbar"
        role="toolbar"
        aria-label={i18nText('agentFlow', 'trajectory.overview')}
      >
        <div className="provider-trajectory__controls">
          <Button
            size="small"
            type="text"
            icon={
              timeScale ? <ClockCircleOutlined /> : <UnorderedListOutlined />
            }
            aria-pressed={timeScale}
            onClick={() => {
              setTimeScale(!timeScale);
              setTimeRange(null);
            }}
          >
            {timeScale
              ? i18nText('agentFlow', 'trajectory.time_axis')
              : i18nText('agentFlow', 'trajectory.sequence_axis')}
          </Button>
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
          <Tooltip
            title={i18nText('agentFlow', 'trajectory.semantic_integrity')}
          >
            <span
              className="provider-trajectory__integrity"
              data-status={overview?.integrity}
            >
              {i18nText('agentFlow', 'trajectory.semantic_integrity')}:{' '}
              {integrityLabel(overview?.integrity)}
            </span>
          </Tooltip>
        </div>
        <Input
          size="small"
          className="provider-trajectory__search"
          prefix={<SearchOutlined />}
          allowClear
          value={search}
          aria-label={i18nText('agentFlow', 'trajectory.search')}
          placeholder={i18nText('agentFlow', 'trajectory.search')}
          onChange={(event) => setSearch(event.target.value)}
        />
      </div>
      <TrajectoryTimeline
        points={items.map((step, index) => ({
          id: step.event_id,
          value: timeScale ? Date.parse(step.created_at) : index,
          lane: stepLane(step),
          label: i18nText('agentFlow', 'trajectory.locate_step', {
            sequence: step.event_sequence,
            kind: stepLabel(step)
          })
        }))}
        range={timeRange}
        onChange={setTimeRange}
        selected={selected}
        timeScale={timeScale}
        onSelect={(id) => {
          const step = items.find((item) => item.event_id === id);
          if (step) {
            setSearch('');
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
                <section key={key}>
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
                    <span>
                      {steps[0].metadata.node_id ||
                        steps[0].metadata.node_run_id}
                    </span>
                    <span className="provider-trajectory__group-id">
                      {steps[0].metadata.invocation_id} ·{' '}
                      {i18nText('agentFlow', 'trajectory.attempt', {
                        count: steps[0].metadata.provider_attempt_index
                      })}
                    </span>
                    <span>{purposeLabel(steps[0].metadata.purpose)}</span>
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
            data-lane={stepLane(selectedStep)}
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
                {stepLabel(selectedStep)}
              </span>
              <span className="provider-trajectory__row-time">
                #{selectedStep.event_sequence}
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
            <TrajectoryStepDetail
              key={selectedStep.event_id}
              step={selectedStep}
              loader={loader}
              onClient={onClient}
            />
          </aside>
        ) : null}
      </div>
      <footer className="provider-trajectory__footer">
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
        <span>
          {i18nText('agentFlow', 'trajectory.protocol_integrity')}:{' '}
          {integrityLabel(overview?.protocol_integrity)}
        </span>
        {overview?.persist_failed_count ? (
          <span>
            {i18nText('agentFlow', 'trajectory.failed_records', {
              count: overview.persist_failed_count
            })}
          </span>
        ) : null}
      </footer>
    </div>
  );
}
