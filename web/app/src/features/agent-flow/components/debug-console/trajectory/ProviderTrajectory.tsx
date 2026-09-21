import { Fragment, useMemo, useRef, useState, type CSSProperties } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Input, Modal, Spin, Tooltip } from 'antd';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import ClockCircleOutlined from '@ant-design/icons/es/icons/ClockCircleOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import UnorderedListOutlined from '@ant-design/icons/es/icons/UnorderedListOutlined';
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';
import { useWindowWorkspaceOverlayZIndex } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { TrajectoryStepDetail } from './TrajectoryStepDetail';
import {
  integrityLabel,
  invocationKey,
  stepLabel,
  stepLane
} from './trajectory-presentation';
import './provider-trajectory.css';

/** A trajectory augments the existing workflow tree; it never owns that tree. */
export function ProviderTrajectory({
  runId,
  nodeRunId,
  loader,
  compatibility_mode
}: {
  runId: string;
  nodeRunId?: string;
  compatibility_mode?: string;
  loader: ConversationLogTraceLoader;
}) {
  const zIndex = useWindowWorkspaceOverlayZIndex();
  const [open, setOpen] = useState(false);
  const title = nodeRunId
    ? i18nText('agentFlow', 'trajectory.title')
    : i18nText('agentFlow', 'trajectory.run_title');
  return (
    <>
      <Tooltip title={title}>
        <Button
          size="small"
          type="text"
          icon={<ApartmentOutlined />}
          aria-label={title}
          title={title}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(true);
          }}
        />
      </Tooltip>
      <Modal
        zIndex={zIndex}
        open={open}
        onCancel={() => setOpen(false)}
        footer={null}
        width="min(1440px, calc(100vw - 32px))"
        title={compatibility_mode ? `${title} · ${compatibility_mode}` : title}
        destroyOnHidden
        styles={{ body: { padding: 0, minHeight: 0 } }}
      >
        {open ? (
          <TrajectoryWorkspace
            key={`${runId}:${nodeRunId ?? 'run'}`}
            runId={runId}
            nodeRunId={nodeRunId}
            loader={loader}
          />
        ) : null}
      </Modal>
    </>
  );
}

function TrajectoryWorkspace({
  runId,
  nodeRunId,
  loader
}: {
  runId: string;
  nodeRunId?: string;
  loader: ConversationLogTraceLoader;
}) {
  const [selected, setSelected] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [timeScale, setTimeScale] = useState(false);
  const [groupCalls, setGroupCalls] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const pages = useInfiniteQuery({
    queryKey: ['provider-trajectory', runId, nodeRunId ?? 'run'],
    enabled: Boolean(
      nodeRunId ? loader.loadTrajectory : loader.loadRunTrajectory
    ),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      nodeRunId
        ? loader.loadTrajectory!(runId, nodeRunId, pageParam)
        : loader.loadRunTrajectory!(runId, pageParam),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchOnWindowFocus: false
  });
  const items = useMemo(
    () => pages.data?.pages.flatMap((page) => page.items) ?? [],
    [pages.data]
  );
  const overview = pages.data?.pages[0];
  const query = search.trim().toLocaleLowerCase();
  const matches = items.filter(
    (step) =>
      !query ||
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
        .includes(query)
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
  const timestamps = items.map((step) => Date.parse(step.created_at));
  const validTimes = timestamps.filter(Number.isFinite);
  const start = validTimes.length ? Math.min(...validTimes) : 0;
  const range = validTimes.length ? Math.max(...validTimes) - start : 0;
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
      <Fragment key={step.event_id}>
        <button
          ref={(element) => {
            if (element) rows.current.set(step.event_id, element);
            else rows.current.delete(step.event_id);
          }}
          type="button"
          className="provider-trajectory__row"
          data-lane={stepLane(step)}
          data-selected={isSelected || undefined}
          aria-label={stepLabel(step)}
          aria-expanded={isSelected}
          onClick={() => setSelected(isSelected ? null : step.event_id)}
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
          {isSelected ? <DownOutlined /> : <RightOutlined />}
        </button>
        {isSelected ? (
          <TrajectoryStepDetail
            key={step.event_id}
            step={step}
            loader={loader}
          />
        ) : null}
      </Fragment>
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
            onClick={() => setTimeScale(!timeScale)}
          >
            {timeScale
              ? i18nText('agentFlow', 'trajectory.time_axis')
              : i18nText('agentFlow', 'trajectory.sequence_axis')}
          </Button>
          <Button
            size="small"
            type="text"
            icon={<ApartmentOutlined />}
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
      <div
        className="provider-trajectory__timeline"
        role="navigation"
        aria-label={i18nText('agentFlow', 'trajectory.timeline')}
      >
        <div className="provider-trajectory__lane-labels">
          <span>{i18nText('agentFlow', 'trajectory.lane_input')}</span>
          <span>{i18nText('agentFlow', 'trajectory.lane_model')}</span>
          <span>{i18nText('agentFlow', 'trajectory.lane_tool')}</span>
        </div>
        <div className="provider-trajectory__lanes">
          {items.map((step, index) => {
            const time = Date.parse(step.created_at);
            const position =
              timeScale && range > 0 && Number.isFinite(time)
                ? ((time - start) / range) * 97
                : (index / Math.max(items.length, 1)) * 100;
            return (
              <Tooltip
                key={step.event_id}
                title={`${stepLabel(step)} · ${formatDateTime(step.created_at)}`}
              >
                <button
                  type="button"
                  className="provider-trajectory__block"
                  data-lane={stepLane(step)}
                  data-selected={selected === step.event_id || undefined}
                  data-dimmed={
                    Boolean(query && !matches.includes(step)) || undefined
                  }
                  aria-label={i18nText('agentFlow', 'trajectory.locate_step', {
                    sequence: step.event_sequence,
                    kind: stepLabel(step)
                  })}
                  style={
                    {
                      '--trajectory-x': `${position}%`,
                      '--trajectory-width': `${Math.min(3, 72 / Math.max(items.length, 1))}%`
                    } as CSSProperties
                  }
                  onClick={() => {
                    setSearch('');
                    focusStep(step);
                  }}
                />
              </Tooltip>
            );
          })}
        </div>
      </div>
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
              <Button onClick={() => void pages.refetch()}>
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
                    {steps[0].metadata.node_id || steps[0].metadata.node_run_id}
                  </span>
                  <span className="provider-trajectory__group-id">
                    {steps[0].metadata.invocation_id}
                  </span>
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
        {pages.hasNextPage ? (
          <div className="provider-trajectory__more">
            <Button
              type="text"
              loading={pages.isFetchingNextPage}
              onClick={() => void pages.fetchNextPage()}
            >
              {i18nText('agentFlow', 'trajectory.more')}
            </Button>
          </div>
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
