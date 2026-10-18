import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Alert, Button, Empty, Input, Spin } from 'antd';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import type {
  ClientTrajectoryIndexItem,
  ClientTrajectoryStep,
  ClientTrajectoryPage,
  ClientTrajectoryOptions
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../../shared/i18n/format';
import { ClientTrajectoryDetail } from './ClientTrajectoryDetail';
import { categoryLabel, clientLane } from './presentation';
import { integrityLabel } from '../trajectory-presentation';
import {
  TrajectoryTimeline,
  inTrajectoryRange,
  type TrajectoryRange
} from '../TrajectoryTimeline';
import { useTrajectoryWindow } from '../virtual/use-trajectory-window';
import {
  TrajectoryVirtualList,
  type TrajectoryVirtualListRef
} from '../virtual/TrajectoryVirtualList';
import {
  indexLane,
  PendingClientRow,
  clientRowKey,
  virtualClientKey,
  virtualClientEventKey
} from './virtual-client-rows';
import { useTranslation } from 'react-i18next';
import './client-trajectory.css';

type Slot =
  | {
      key: string;
      kind: 'request';
      id: string;
      root?: ClientTrajectoryIndexItem;
    }
  | {
      key: string;
      kind: 'category';
      id: string;
      category: string;
      count: number;
    }
  | {
      key: string;
      kind: 'step';
      step: ClientTrajectoryIndexItem;
      grouped: boolean;
    };
const EMPTY_INDEX: ClientTrajectoryIndexItem[] = [];
export function ClientTrajectoryWorkspace({
  runId,
  nodeRunId,
  loader,
  options,
  active = true,
  onInternal
}: {
  active?: boolean;
  runId: string;
  nodeRunId?: string;
  loader: ConversationLogTraceLoader;
  options?: ClientTrajectoryOptions;
  onInternal?: (step: ClientTrajectoryStep, nodeRunId?: string) => void;
}) {
  const { i18n } = useTranslation('agentFlow');
  const language = i18n.resolvedLanguage ?? i18n.language;
  const [scope, setScope] = useState(nodeRunId);
  const [selected, setSelected] = useState<string | null>(
    options?.focus_step_id ?? null
  );
  const [timeScale, setTimeScale] = useState(true);
  const [timeRange, setTimeRange] = useState<TrajectoryRange>(null);
  const [brushItems, setBrushItems] = useState<
    ClientTrajectoryIndexItem[] | null
  >(null);
  const [search, setSearch] = useState('');
  const [groupCategories, setGroupCategories] = useState(false);
  const [category, setCategory] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(new Set<string>());
  const [detailWidth, setDetailWidth] = useState<number | null>(null);
  const split = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const virtualList = useRef<TrajectoryVirtualListRef>(null);
  const [generation, setGeneration] = useState(0);
  const windowQuery = {
    category: category ?? undefined,
    keyword: search.trim() || undefined,
    ...(timeScale && timeRange
      ? {
          from: new Date(Math.floor(timeRange[0])).toISOString(),
          to: new Date(Math.floor(timeRange[1]) + 1).toISOString()
        }
      : {})
  };
  const {
    overview: indexQuery,
    pages,
    loaded,
    items,
    initialKey,
    onVisibleKeys,
    requestFocus,
    retry
  } = useTrajectoryWindow<
    ClientTrajectoryStep,
    ClientTrajectoryIndexItem,
    ClientTrajectoryPage
  >({
    queryKey: [
      'client-trajectory',
      runId,
      scope ?? 'run',
      options,
      windowQuery,
      generation
    ],
    enabled: active && Boolean(loader.loadClientTrajectory),
    loadPage: (focusId, indexOnly) => {
      const requestOptions = {
        ...options,
        ...(indexOnly ? windowQuery : {}),
        focus_step_id: focusId ?? options?.focus_step_id,
        index_only: indexOnly
      };
      return loader.loadClientTrajectory!(
        runId,
        scope,
        undefined,
        requestOptions
      );
    },
    getIndexKey: clientRowKey,
    getRowKey: clientRowKey
  });
  const overview = indexQuery.data?.overview;
  const index = overview?.items ?? EMPTY_INDEX;
  const timelineItems = brushItems ?? index;
  const coordinates = useMemo(
    () =>
      new Map(
        timelineItems.map((step, position) => [
          step.id,
          timeScale ? Date.parse(step.created_at) : position
        ])
      ),
    [timelineItems, timeScale]
  );
  const timelinePoints = useMemo(() => {
    const labels = new Map<string, string>();
    return timelineItems.map((step) => {
      if (!labels.has(step.category))
        labels.set(step.category, categoryLabel(step.category, language));
      return {
        id: step.id,
        value: coordinates.get(step.id)!,
        lane: indexLane(step),
        label: `${labels.get(step.category)} #${step.sequence}`
      };
    });
  }, [timelineItems, coordinates, language]);
  const selectedStep = selected ? loaded.get(selected) : undefined;
  const categories = Object.keys(overview?.category_counts ?? {});
  const requestRoots = useMemo(
    () =>
      new Map(
        (overview?.request_roots ?? []).map((step) => [step.request_id, step])
      ),
    [overview]
  );
  const requests = useMemo(() => {
    const groups = new Map<string, ClientTrajectoryIndexItem[]>();
    for (const step of index) {
      const group = groups.get(step.request_id) ?? [];
      group.push(step);
      groups.set(step.request_id, group);
    }
    return [...groups.entries()];
  }, [index]);
  const slots = useMemo(() => {
    const matches = (step: ClientTrajectoryIndexItem) =>
      timeScale || inTrajectoryRange(coordinates.get(step.id)!, timeRange);
    const slots: Slot[] = [];
    for (const [id, steps] of requests) {
      const root =
        steps.find((step) => step.category === 'request') ??
        requestRoots.get(id);
      const children = steps.filter(
        (step) => step.category !== 'request' && matches(step)
      );
      if (
        !children.length &&
        !steps.some((step) => step.category === 'request' && matches(step))
      )
        continue;
      slots.push({
        key: root?.id ?? `request:${id}`,
        kind: 'request',
        id,
        root
      });
      if (collapsed.has(id)) continue;
      if (!groupCategories) {
        for (const step of children)
          slots.push({ key: step.id, kind: 'step', step, grouped: false });
      } else {
        for (const lane of new Set(children.map((step) => step.category))) {
          const key = `${id}:${lane}`;
          const laneRows = children.filter((step) => step.category === lane);
          slots.push({
            key: `category:${key}`,
            kind: 'category',
            id: key,
            category: lane,
            count: laneRows.length
          });
          if (!collapsed.has(key))
            for (const step of laneRows)
              slots.push({ key: step.id, kind: 'step', step, grouped: true });
        }
      }
    }
    return slots;
  }, [
    requests,
    requestRoots,
    coordinates,
    timeScale,
    timeRange,
    collapsed,
    groupCategories
  ]);
  const focus = useCallback(
    (id: string) => {
      const step =
        timelineItems.find((item) => item.id === id) ??
        overview?.request_roots?.find((item) => item.id === id);
      setSelected(id);
      setCategory(null);
      setSearch('');
      requestFocus(id);
      if (step)
        setCollapsed((current) => {
          const next = new Set(current);
          next.delete(step.request_id);
          next.delete(`${step.request_id}:${step.category}`);
          return next;
        });
      requestAnimationFrame(() => virtualList.current?.scrollTo({ key: id }));
    },
    [timelineItems, overview, requestFocus]
  );
  useEffect(() => {
    if (!active || !selected || !index.some((step) => step.id === selected))
      return;
    requestFocus(selected);
    requestAnimationFrame(() =>
      virtualList.current?.scrollTo({ key: selected })
    );
  }, [active, selected, index, requestFocus]);
  const toggle = useCallback((id: string) => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  function resize(width: number) {
    setDetailWidth(
      Math.max(
        320,
        Math.min(720, (split.current?.clientWidth ?? 1000) - 280, width)
      )
    );
  }
  const renderStep = useCallback(
    (step: ClientTrajectoryStep) => {
      return (
        <button
          key={step.id}
          type="button"
          className="provider-trajectory__row client-trajectory__row"
          data-lane={clientLane(step)}
          data-selected={selected === step.id || undefined}
          aria-label={`${categoryLabel(step.category, language)} · ${step.namespace ? `${step.namespace}.${step.name}` : step.name}`}
          aria-pressed={selected === step.id}
          onClick={() => setSelected(step.id)}
        >
          <span className="provider-trajectory__marker" aria-hidden="true" />
          <span className="provider-trajectory__kind">
            {categoryLabel(step.category, language)}
          </span>
          <span className="client-trajectory__summary">
            <strong>
              {step.namespace ? `${step.namespace}.${step.name}` : step.name}
            </strong>
            <span className="provider-trajectory__preview">
              {step.parameters_preview ||
                step.result_preview ||
                step.preview ||
                '—'}
            </span>
          </span>
          <span
            className="provider-trajectory__row-time"
            title={formatDateTime(step.created_at)}
          >
            #{step.sequence}
          </span>
        </button>
      );
    },
    [selected, language]
  );
  const renderSlot = useCallback(
    (slot: Slot) => {
      if (slot.kind === 'step') {
        const row = loaded.get(slot.step.id);
        const content = row ? (
          renderStep(row)
        ) : (
          <PendingClientRow
            step={slot.step}
            selected={selected === slot.step.id}
            onSelect={() => focus(slot.step.id)}
          />
        );
        return slot.grouped ? (
          <div className="client-trajectory__category-group">{content}</div>
        ) : (
          content
        );
      }
      if (slot.kind === 'category')
        return (
          <div className="client-trajectory__category-group">
            <button
              className="client-trajectory__category-header"
              type="button"
              aria-expanded={!collapsed.has(slot.id)}
              onClick={() => toggle(slot.id)}
            >
              {collapsed.has(slot.id) ? <RightOutlined /> : <DownOutlined />}
              <span>{categoryLabel(slot.category, language)}</span>
              <small>{slot.count}</small>
            </button>
          </div>
        );
      const root = slot.root
        ? slot.root
        : items.find(
            (step) => step.request_id === slot.id && step.category === 'request'
          );
      const fullRoot = root ? loaded.get(root.id) : undefined;
      return (
        <div className="client-trajectory__request-header">
          <button
            type="button"
            className="client-trajectory__fold"
            aria-label={i18nText('agentFlow', 'client_trajectory.request')}
            aria-expanded={!collapsed.has(slot.id)}
            onClick={() => toggle(slot.id)}
          >
            {collapsed.has(slot.id) ? <RightOutlined /> : <DownOutlined />}
          </button>
          <button
            type="button"
            className="client-trajectory__request-select"
            aria-pressed={selected === root?.id}
            onClick={() => root && focus(root.id)}
          >
            <strong>{categoryLabel('request', language)}</strong>
            <span>
              {fullRoot?.preview || (
                <>
                  <Spin size="small" />{' '}
                  {i18nText('agentFlow', 'trajectory.loading_first_page')}
                </>
              )}
            </span>
            <small>{root ? formatDateTime(root.created_at) : ''}</small>
          </button>
        </div>
      );
    },
    [loaded, items, selected, collapsed, renderStep, focus, toggle, language]
  );
  return (
    <div className="provider-trajectory client-trajectory">
      <div className="provider-trajectory__toolbar">
        <div className="provider-trajectory__controls">
          <span className="client-trajectory__protocol">
            {[...new Set(index.map((step) => step.protocol))]
              .map((protocol) =>
                protocol === 'chat_completions'
                  ? 'Chat Completions'
                  : protocol === 'responses'
                    ? 'Responses'
                    : protocol
              )
              .join(' · ')}
          </span>
          <Button
            size="small"
            type="text"
            aria-pressed={timeScale}
            onClick={() => {
              setTimeScale(!timeScale);
              setTimeRange(null);
              setBrushItems(null);
            }}
          >
            {i18nText(
              'agentFlow',
              timeScale ? 'trajectory.time_axis' : 'trajectory.sequence_axis'
            )}
          </Button>
          <Button
            size="small"
            type="text"
            aria-pressed={groupCategories}
            onClick={() => setGroupCategories(!groupCategories)}
          >
            {i18nText('agentFlow', 'client_trajectory.group_categories')}
          </Button>
          <span
            className="provider-trajectory__integrity"
            data-status={pages.data?.pages[0]?.integrity}
          >
            {integrityLabel(
              pages.data?.pages[0]?.integrity as Parameters<
                typeof integrityLabel
              >[0]
            )}
          </span>
          <Button
            size="small"
            type="text"
            onClick={() => setCollapsed(new Set())}
          >
            {i18nText('agentFlow', 'client_trajectory.expand')}
          </Button>
          <Button
            size="small"
            type="text"
            onClick={() => setCollapsed(new Set(requests.map(([id]) => id)))}
          >
            {i18nText('agentFlow', 'client_trajectory.collapse')}
          </Button>
          <Button
            size="small"
            type="text"
            loading={indexQuery.isFetching || pages.isFetching}
            onClick={() => {
              setGeneration((value) => value + 1);
            }}
          >
            {i18nText('agentFlow', 'client_trajectory.refresh')}
          </Button>
        </div>
        <Input
          className="provider-trajectory__search"
          size="small"
          prefix={<SearchOutlined />}
          allowClear
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          aria-label={i18nText('agentFlow', 'trajectory.search')}
          placeholder={i18nText('agentFlow', 'trajectory.search')}
        />
      </div>
      <TrajectoryTimeline
        points={timelinePoints}
        timeScale={timeScale}
        range={timeRange}
        onChange={(range) => {
          setTimeRange(range);
          setBrushItems(range ? timelineItems : null);
        }}
        selected={selected}
        onSelect={focus}
      />
      <div className="provider-trajectory__split" ref={split}>
        <div className="provider-trajectory__ledger client-trajectory__virtual-ledger">
          <nav
            className="client-trajectory__categories"
            aria-label={i18nText('agentFlow', 'client_trajectory.categories')}
          >
            <button
              type="button"
              aria-pressed={!category}
              onClick={() => setCategory(null)}
            >
              {i18nText('agentFlow', 'client_trajectory.all')}
            </button>
            {categories.map((key) => (
              <button
                type="button"
                key={key}
                aria-pressed={category === key}
                onClick={() => setCategory(category === key ? null : key)}
              >
                {categoryLabel(key)}{' '}
                <span>{overview?.category_counts[key] ?? 0}</span>
              </button>
            ))}
          </nav>
          {scope ? (
            <div className="client-trajectory__note">
              {i18nText('agentFlow', 'client_trajectory.node_scope')}{' '}
              <Button
                type="link"
                size="small"
                onClick={() => {
                  setScope(undefined);
                  setSelected(null);
                }}
              >
                {i18nText('agentFlow', 'client_trajectory.open_run')}
              </Button>
            </div>
          ) : null}
          {indexQuery.isLoading || pages.isLoading ? (
            <div className="provider-trajectory__loading" role="status">
              <Spin />
              <span>
                {i18nText('agentFlow', 'trajectory.loading_first_page')}
              </span>
            </div>
          ) : null}
          {indexQuery.isError || pages.isError ? (
            <Alert
              type="error"
              title={i18nText('agentFlow', 'auto.loading_failed')}
              action={
                <Button onClick={retry}>
                  {i18nText('agentFlow', 'auto.retry')}
                </Button>
              }
            />
          ) : null}
          {!(indexQuery.isLoading || pages.isLoading) &&
          !(indexQuery.isError || pages.isError) &&
          !slots.length ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText(
                'agentFlow',
                'client_trajectory.no_records'
              )}
            />
          ) : null}
          <TrajectoryVirtualList
            ref={virtualList}
            items={slots}
            getKey={virtualClientKey}
            getEventKey={virtualClientEventKey}
            initialKey={initialKey}
            resetKey={`${runId}:${scope ?? 'run'}:${generation}`}
            active={active}
            onVisibleKeys={onVisibleKeys}
            renderItem={renderSlot}
          />
        </div>
        {selectedStep ? (
          <aside
            className="provider-trajectory__inspector"
            data-lane={clientLane(selectedStep)}
            aria-label={i18nText('agentFlow', 'trajectory.inspector')}
            style={detailWidth === null ? undefined : { width: detailWidth }}
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
                  resize(drag.current.width + drag.current.x - event.clientX);
              }}
              onPointerUp={() => {
                drag.current = null;
              }}
              onPointerCancel={() => {
                drag.current = null;
              }}
              onKeyDown={(event) => {
                if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
                  resize(
                    event.currentTarget.parentElement!.getBoundingClientRect()
                      .width + (event.key === 'ArrowLeft' ? 16 : -16)
                  );
                  event.preventDefault();
                }
              }}
            />
            <div className="provider-trajectory__inspector-header">
              <span className="provider-trajectory__kind">
                {categoryLabel(selectedStep.category)}
              </span>
              <strong className="provider-trajectory__preview">
                {selectedStep.namespace
                  ? `${selectedStep.namespace}.${selectedStep.name}`
                  : selectedStep.name}
              </strong>
              <Button
                size="small"
                type="text"
                icon={<CloseOutlined />}
                aria-label={i18nText('agentFlow', 'client_trajectory.close')}
                onClick={() => {
                  setSelected(null);
                  virtualList.current?.focusKey(selectedStep.id);
                }}
              />
            </div>
            {onInternal && selectedStep.flow_run_id ? (
              <Button
                type="link"
                onClick={() => onInternal(selectedStep, scope)}
              >
                {i18nText('agentFlow', 'trajectory.open_internal_calls')}
              </Button>
            ) : null}
            <ClientTrajectoryDetail
              key={selectedStep.id}
              step={selectedStep}
              runId={runId}
              loader={loader}
              nodeRunId={scope}
              onRelated={(id) => {
                setTimeRange(null);
                setBrushItems(null);
                focus(id);
              }}
            />
          </aside>
        ) : null}
      </div>
      <footer className="provider-trajectory__footer">
        <span role="status">
          {indexQuery.isError || pages.isError
            ? i18nText('agentFlow', 'auto.loading_failed')
            : indexQuery.isPending || pages.isPending
              ? i18nText('agentFlow', 'trajectory.loading_first_page')
              : indexQuery.isFetching || pages.isFetching
                ? i18nText('agentFlow', 'trajectory.loading_pages')
                : i18nText('agentFlow', 'trajectory.loading_complete')}
        </span>
        <span>
          {i18nText('agentFlow', 'trajectory.loaded_steps', {
            count: items.length
          })}
        </span>
        <span>{i18nText('agentFlow', 'client_trajectory.client')}</span>
      </footer>
    </div>
  );
}
