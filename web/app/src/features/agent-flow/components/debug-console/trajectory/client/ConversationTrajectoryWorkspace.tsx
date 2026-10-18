import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Input, Popover, Select, Spin } from 'antd';
import type {
  ConversationTrajectoryQuery,
  ConversationClientTrajectoryQueryPage,
  ConversationTrajectoryIndexItem,
  ConversationClientTrajectoryStep
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../../shared/i18n/format';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import FilterOutlined from '@ant-design/icons/es/icons/FilterOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import { TrajectoryTimeline } from '../TrajectoryTimeline';
import { type TrajectoryRange } from '../timeline-model';
import { useTrajectoryWindow } from '../virtual/use-trajectory-window';
import {
  TrajectoryVirtualList,
  type TrajectoryVirtualListRef
} from '../virtual/TrajectoryVirtualList';
import { PendingClientRow } from './virtual-client-rows';
import { indexLane, clientRowKey, virtualClientKey, virtualClientEventKey } from './virtual-client-model';
import { useTranslation } from 'react-i18next';
import { integrityLabel } from '../trajectory-presentation';
import { ClientTrajectoryDetail } from './ClientTrajectoryDetail';
import { categoryLabel, clientLane, sectionLabel } from './presentation';
import './client-trajectory.css';
const text = (key: string, language?: string) =>
  (
    ({
      missing_identity: i18nText('agentFlow', 'conversation_trajectory.missing_identity', { lng: language }),
      provenance: i18nText('agentFlow', 'conversation_trajectory.provenance', { lng: language }),
      all_sources: i18nText('agentFlow', 'conversation_trajectory.all_sources', { lng: language }),
      main: i18nText('agentFlow', 'conversation_trajectory.main', { lng: language }),
      subagent: i18nText('agentFlow', 'conversation_trajectory.subagent', { lng: language }),
      from: i18nText('agentFlow', 'conversation_trajectory.from', { lng: language }),
      to: i18nText('agentFlow', 'conversation_trajectory.to', { lng: language }),
      search_sections: i18nText('agentFlow', 'conversation_trajectory.search_sections', { lng: language }),
      loaded: i18nText('agentFlow', 'conversation_trajectory.loaded', { lng: language }),
      search: i18nText('agentFlow', 'conversation_trajectory.search', { lng: language }),
      invalid_time: i18nText('agentFlow', 'conversation_trajectory.invalid_time', { lng: language }),
      filters: i18nText('agentFlow', 'conversation_trajectory.filters', { lng: language }),
      loaded_categories: i18nText('agentFlow', 'conversation_trajectory.loaded_categories', { lng: language }),
      loaded_positions: i18nText('agentFlow', 'conversation_trajectory.loaded_positions', { lng: language }),
      subagent_marker: i18nText('agentFlow', 'conversation_trajectory.subagent_marker', { lng: language }),
      related_unloaded: i18nText('agentFlow', 'conversation_trajectory.related_unloaded', { lng: language })
    }) as Record<string, string>
  )[key];
/** Canonical identity comes from the server; a record is only an initial focus. */
type Slot =
  | { key: string; kind: 'step'; step: ConversationTrajectoryIndexItem }
  | { key: string; kind: 'category'; category: string; count: number };
const EMPTY_INDEX: ConversationTrajectoryIndexItem[] = [];
export function ConversationTrajectoryWorkspace({
  loader,
  recordId,
  logConversationId,
  active = true
}: {
  loader: ConversationLogTraceLoader;
  recordId: string;
  logConversationId?: string | null;
  active?: boolean;
}) {
  const overview = useQuery({
    queryKey: ['conversation-trajectory-identity', recordId],
    enabled:
      active &&
      !logConversationId &&
      Boolean(loader.loadRecordOverviewForTrajectory),
    queryFn: () => loader.loadRecordOverviewForTrajectory!(recordId),
    refetchOnWindowFocus: false
  });
  const canonical = logConversationId ?? overview.data?.log_conversation_id;
  if (canonical)
    return (
      <CanonicalTimeline
        key={`${canonical}:${recordId}`}
        loader={loader}
        logConversationId={canonical}
        focusRecordId={recordId}
        active={active}
      />
    );
  if (overview.isLoading) return <Spin />;
  if (overview.isError)
    return (
      <Alert
        type="error"
        title={i18nText('agentFlow', 'auto.loading_failed')}
        action={
          <Button onClick={() => void overview.refetch()}>
            {i18nText('agentFlow', 'auto.retry')}
          </Button>
        }
      />
    );
  return <Alert type="warning" title={text('missing_identity')} />;
}
function timestamp(value: string): string | undefined {
  if (!value) return undefined;
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date.toISOString() : undefined;
}
function CanonicalTimeline({
  loader,
  logConversationId,
  focusRecordId,
  active
}: {
  loader: ConversationLogTraceLoader;
  logConversationId: string;
  focusRecordId: string;
  active: boolean;
}) {
  const { i18n } = useTranslation('agentFlow');
  const language = i18n.resolvedLanguage ?? i18n.language;
  const [category, setCategory] = useState<string>();
  const [timeScale, setTimeScale] = useState(true);
  const [timeRange, setTimeRange] = useState<TrajectoryRange>(null);
  const [brushItems, setBrushItems] = useState<
    ConversationTrajectoryIndexItem[] | null
  >(null);
  const preserveRangeOnSelect = useRef(false);
  const pointerStart = useRef<number | null>(null);
  const pointerMoved = useRef(false);
  const committedSelection = useRef<{
    range: TrajectoryRange;
    items: ConversationTrajectoryIndexItem[] | null;
  }>({ range: null, items: null });
  const pendingSelection = useRef<{
    range: TrajectoryRange;
    items: ConversationTrajectoryIndexItem[] | null;
  } | null>(null);
  const pendingAxisFilter = useRef<Record<string, string> | null>(null);
  const [axisFilter, setAxisFilter] = useState<Record<string, string> | null>(
    null
  );
  const [groupCategories, setGroupCategories] = useState(false);
  const [seenCategories, setSeenCategories] = useState<string[]>([]);
  const [detailWidth, setDetailWidth] = useState<number | null>(null);
  const split = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const virtualList = useRef<TrajectoryVirtualListRef>(null);
  const [provenance, setProvenance] = useState('all');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [keyword, setKeyword] = useState('');
  const [searchSections, setSearchSections] = useState([
    'overview',
    'parameters',
    'result'
  ]);
  const [selected, setSelected] = useState<string | null>(null);
  const [generation, setGeneration] = useState(0);
  const [relatedTarget, setRelatedTarget] = useState<string | null>(null);
  const [relatedStep, setRelatedStep] =
    useState<ConversationClientTrajectoryStep | null>(null);
  const fromTimestamp = timestamp(from);
  const toTimestamp = timestamp(to);
  const invalidTime = Boolean((from && !fromTimestamp) || (to && !toTimestamp));
  const query: ConversationTrajectoryQuery = {
    filter: {
      ...(category ? { category: { $eq: category } } : {}),
      ...(provenance !== 'all'
        ? { is_subagent: { $eq: provenance === 'subagent' } }
        : {}),
      ...(axisFilter
        ? { created_at: axisFilter }
        : fromTimestamp || toTimestamp
          ? {
              created_at: {
                ...(fromTimestamp ? { $gte: fromTimestamp } : {}),
                ...(toTimestamp ? { $lte: toTimestamp } : {})
              }
            }
          : {})
    },
    keyword: keyword || undefined,
    search_sections: searchSections,
    focus_record_id: focusRecordId,
    limit: 50
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
    ConversationClientTrajectoryStep,
    ConversationTrajectoryIndexItem,
    ConversationClientTrajectoryQueryPage
  >({
    queryKey: [
      'conversation-client-trajectory',
      logConversationId,
      query,
      generation
    ],
    enabled:
      active &&
      !invalidTime &&
      Boolean(loader.queryConversationClientTrajectory),
    loadPage: (focusId, indexOnly) =>
      loader.queryConversationClientTrajectory!(logConversationId, {
        ...query,
        focus_step_id: focusId,
        index_only: indexOnly
      }),
    getIndexKey: clientRowKey,
    getRowKey: clientRowKey
  });
  // Preserve server full-precision order, including duplicate timestamps.
  const scopeOverview = indexQuery.data?.overview;
  const index = scopeOverview?.items ?? EMPTY_INDEX;
  const pageData = pages.data?.pages;
  const matchesByStep = useMemo(() => {
    const matches = new Map<
      string,
      NonNullable<typeof pageData>[number]['matches']
    >();
    for (const page of pageData ?? [])
      for (const match of page.matches) {
        const entries = matches.get(match.step_id) ?? [];
        entries.push(match);
        matches.set(match.step_id, entries);
      }
    return matches;
  }, [pageData]);
  const related = useQuery({
    queryKey: [
      'conversation-trajectory-related',
      logConversationId,
      relatedTarget,
      generation
    ],
    enabled:
      active &&
      Boolean(relatedTarget && loader.queryConversationClientTrajectory),
    queryFn: () =>
      loader.queryConversationClientTrajectory!(logConversationId, {
        filter: { id: { $eq: relatedTarget } },
        limit: 1
      }),
    retry: false,
    refetchOnWindowFocus: false
  });
  useEffect(() => {
    if (relatedTarget && related.data?.items[0])
      setRelatedStep(related.data.items[0]);
  }, [relatedTarget, related.data]);
  const selectedStep =
    relatedStep ?? (selected ? loaded.get(selected) : undefined);
  const timelineItems = brushItems ?? index;
  const coordinates = useMemo(
    () =>
      new Map(
        timelineItems.map((step, index) => [
          step.id,
          timeScale ? Date.parse(step.created_at) : index
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
  useEffect(() => {
    setSeenCategories((current) => [
      ...new Set([
        ...current,
        ...Object.keys(scopeOverview?.category_counts ?? {})
      ])
    ]);
  }, [scopeOverview]);
  const select = useCallback(
    (id: string) => {
      preserveRangeOnSelect.current = false;
      if (index.some((step) => step.id === id)) {
        setSelected(id);
        setRelatedStep(null);
        setRelatedTarget(null);
        requestFocus(id);
        requestAnimationFrame(() => virtualList.current?.scrollTo({ key: id }));
      } else setRelatedTarget(id);
    },
    [index, requestFocus]
  );
  function changeRange(range: TrajectoryRange) {
    if (!range && preserveRangeOnSelect.current) return;
    setTimeRange(range);
    if (!range) {
      pendingAxisFilter.current = null;
      pendingSelection.current = null;
      committedSelection.current = { range: null, items: null };
      setAxisFilter(null);
      setBrushItems(null);
      return;
    }
    setBrushItems(timelineItems);
    pendingSelection.current = { range, items: timelineItems };
    if (timeScale) {
      // Inclusive display milliseconds include persisted sub-millisecond events.
      pendingAxisFilter.current = {
        $gte: new Date(Math.floor(range[0])).toISOString(),
        $lt: new Date(Math.floor(range[1]) + 1).toISOString()
      };
    } else {
      const first =
        timelineItems[
          Math.max(0, Math.min(timelineItems.length - 1, Math.ceil(range[0])))
        ];
      const last =
        timelineItems[
          Math.max(0, Math.min(timelineItems.length - 1, Math.floor(range[1])))
        ];
      if (first && last)
        pendingAxisFilter.current =
          Math.ceil(range[0]) > Math.floor(range[1])
            ? { $gt: first.created_at, $lte: first.created_at }
            : { $gte: first.created_at, $lte: last.created_at };
    }
  }
  function commitRange() {
    if (pendingAxisFilter.current) {
      setAxisFilter(pendingAxisFilter.current);
      if (pendingSelection.current)
        committedSelection.current = pendingSelection.current;
      pendingAxisFilter.current = null;
      pendingSelection.current = null;
    }
  }
  function cancelRange() {
    pendingAxisFilter.current = null;
    pendingSelection.current = null;
    pointerStart.current = null;
    pointerMoved.current = false;
    preserveRangeOnSelect.current = false;
    setTimeRange(committedSelection.current.range);
    setBrushItems(committedSelection.current.items);
  }
  function resize(width: number) {
    setDetailWidth(
      Math.max(
        320,
        Math.min(720, (split.current?.clientWidth ?? 1000) - 280, width)
      )
    );
  }
  function refresh() {
    setSelected(null);
    setRelatedTarget(null);
    setRelatedStep(null);
    setBrushItems(null);
    setGeneration((value) => value + 1);
  }
  const renderStep = useCallback(
    (step: ConversationClientTrajectoryStep) => {
      const matches = matchesByStep.get(step.id) ?? [];
      const identity = `${step.is_subagent ? text('subagent', language) : text('main', language)} · ${step.source_client ?? step.source_kind} · ${step.source_session_id ?? '—'} · ${step.source_task_id ?? '—'} · ${step.source_id ?? '—'} · ${step.record_id}`;
      return (
        <button
          key={`${step.record_id}:${step.id}`}
          type="button"
          className="provider-trajectory__row client-trajectory__row conversation-trajectory__row"
          data-lane={clientLane(step)}
          data-selected={selectedStep?.id === step.id || undefined}
          aria-pressed={selectedStep?.id === step.id}
          aria-label={`${categoryLabel(step.category, language)} · ${step.name}`}
          onClick={() => select(step.id)}
        >
          <span className="provider-trajectory__marker" aria-hidden="true" />
          <span className="provider-trajectory__kind">
            {categoryLabel(step.category, language)}
          </span>
          <span className="client-trajectory__summary">
            <strong>
              {step.namespace ? `${step.namespace}.${step.name}` : step.name}
            </strong>
            <span
              className="provider-trajectory__preview"
              title={
                matches
                  .map(
                    (match) =>
                      `${sectionLabel(match.section, language)} · ${match.snippet}`
                  )
                  .join('\n') || step.preview
              }
            >
              {step.parameters_preview ||
                step.result_preview ||
                step.preview ||
                '—'}
            </span>
            {step.is_subagent ? (
              <small
                className="conversation-trajectory__source"
                title={identity}
              >
                {text('subagent_marker', language)}
              </small>
            ) : null}
          </span>
          <span
            className="provider-trajectory__row-time"
            title={`${formatDateTime(step.created_at)} · ${step.created_at} · ${identity}`}
          >
            #{step.sequence}
          </span>
        </button>
      );
    },
    [matchesByStep, selectedStep?.id, select, language]
  );
  const slots = useMemo(() => {
    const slots: Slot[] = [];
    if (!groupCategories)
      for (const step of index)
        slots.push({ key: step.id, kind: 'step', step });
    else
      for (const key of new Set(index.map((step) => step.category))) {
        const categoryRows = index.filter((step) => step.category === key);
        slots.push({
          key: `category:${key}`,
          kind: 'category',
          category: key,
          count: categoryRows.length
        });
        for (const step of categoryRows)
          slots.push({ key: step.id, kind: 'step', step });
      }
    return slots;
  }, [index, groupCategories]);
  const renderSlot = useCallback(
    (slot: Slot) => {
      if (slot.kind === 'category')
        return (
          <div className="client-trajectory__category-group">
            <div className="client-trajectory__category-header">
              <span>{categoryLabel(slot.category, language)}</span>
              <small>{slot.count}</small>
            </div>
          </div>
        );
      const row = loaded.get(slot.step.id);
      const content = row ? (
        renderStep(row)
      ) : (
        <PendingClientRow
          step={slot.step}
          selected={selected === slot.step.id}
          onSelect={() => select(slot.step.id)}
        />
      );
      return groupCategories ? (
        <div className="client-trajectory__category-group">{content}</div>
      ) : (
        content
      );
    },
    [loaded, selected, groupCategories, renderStep, select, language]
  );
  const detailLoader = useMemo<ConversationLogTraceLoader>(
    () => ({
      loadClientTrajectorySection: (
        canonical,
        stepId,
        section,
        _node,
        cursor
      ) =>
        loader.loadConversationClientTrajectorySection!(
          canonical,
          stepId,
          section,
          cursor
        )
    }),
    [loader]
  );
  return (
    <div className="provider-trajectory client-trajectory conversation-trajectory">
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
              changeRange(null);
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
            loading={pages.isFetching}
            onClick={refresh}
          >
            {i18nText('agentFlow', 'client_trajectory.refresh')}
          </Button>
          <Popover
            trigger="click"
            placement="bottomRight"
            content={
              <div className="conversation-trajectory__filters">
                <Select
                  size="small"
                  aria-label={i18nText(
                    'agentFlow',
                    'client_trajectory.categories'
                  )}
                  value={category ?? ''}
                  onChange={(value) => setCategory(value || undefined)}
                  options={[
                    {
                      value: '',
                      label: i18nText('agentFlow', 'client_trajectory.all')
                    },
                    ...[
                      ...new Set([
                        'request',
                        'system',
                        'user',
                        'assistant',
                        'reasoning',
                        'tool_definition',
                        'tool_call',
                        'tool_result',
                        'usage',
                        'error',
                        'context',
                        'task_start',
                        'task_end',
                        ...seenCategories
                      ])
                    ].map((value) => ({ value, label: categoryLabel(value) }))
                  ]}
                />
                <label className="conversation-trajectory__time">
                  {text('from')}
                  <input
                    type="datetime-local"
                    step="1"
                    value={from}
                    onChange={(event) => {
                      setFrom(event.target.value);
                      changeRange(null);
                    }}
                  />
                </label>
                <label className="conversation-trajectory__time">
                  {text('to')}
                  <input
                    type="datetime-local"
                    step="1"
                    value={to}
                    onChange={(event) => {
                      setTo(event.target.value);
                      changeRange(null);
                    }}
                  />
                </label>
                <Select
                  size="small"
                  mode="multiple"
                  aria-label={text('search_sections')}
                  value={searchSections}
                  onChange={(values) =>
                    setSearchSections(values.length ? values : ['result'])
                  }
                  options={['overview', 'parameters', 'result'].map(
                    (value) => ({ value, label: sectionLabel(value) })
                  )}
                />
              </div>
            }
          >
            <Button
              size="small"
              type="text"
              icon={<FilterOutlined />}
              aria-label={text('filters')}
            />
          </Popover>
        </div>
        <Input
          className="provider-trajectory__search"
          size="small"
          prefix={<SearchOutlined />}
          aria-label={text('search')}
          allowClear
          onPressEnter={(event) => setKeyword(event.currentTarget.value.trim())}
          onChange={(event) => {
            if (!event.target.value) setKeyword('');
          }}
          placeholder={text('search')}
        />
      </div>
      <div
        className="conversation-trajectory__axis"
        onPointerDownCapture={(event) => {
          pointerStart.current = event.clientX;
          pointerMoved.current = false;
        }}
        onPointerMoveCapture={(event) => {
          if (
            pointerStart.current !== null &&
            Math.abs(event.clientX - pointerStart.current) >= 3
          )
            pointerMoved.current = true;
        }}
        onPointerUpCapture={() => {
          preserveRangeOnSelect.current = !pointerMoved.current;
          pointerStart.current = null;
        }}
        onClickCapture={(event) => {
          preserveRangeOnSelect.current = Boolean(
            (event.target as HTMLElement).closest('.provider-trajectory__block')
          );
        }}
        onPointerUp={commitRange}
        onPointerCancel={cancelRange}
        onKeyDown={(event) => {
          if (
            (event.target as HTMLElement).dataset.brush &&
            ['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Escape'].includes(
              event.key
            )
          )
            commitRange();
        }}
      >
        <TrajectoryTimeline
          points={timelinePoints}
          timeScale={timeScale}
          range={timeRange}
          onChange={changeRange}
          selected={selectedStep?.id ?? selected}
          onSelect={select}
        />
      </div>
      {!timeScale ? (
        <div className="client-trajectory__note">
          {text('loaded_positions')}
        </div>
      ) : null}
      {invalidTime ? (
        <Alert type="warning" title={text('invalid_time')} />
      ) : null}
      <div className="provider-trajectory__split" ref={split}>
        <div className="provider-trajectory__ledger client-trajectory__virtual-ledger">
          <nav
            className="client-trajectory__categories"
            aria-label={text('loaded_categories')}
            title={text('loaded_categories')}
          >
            <button
              type="button"
              aria-pressed={!category}
              onClick={() => setCategory(undefined)}
            >
              {i18nText('agentFlow', 'client_trajectory.all')}{' '}
              <span>{scopeOverview?.total_count ?? 0}</span>
            </button>
            {seenCategories.map((key) => (
              <button
                type="button"
                key={key}
                aria-label={categoryLabel(key)}
                aria-pressed={category === key}
                onClick={() => setCategory(category === key ? undefined : key)}
              >
                {categoryLabel(key)}{' '}
                <span>{scopeOverview?.category_counts[key] ?? 0}</span>
              </button>
            ))}
            <span
              className="conversation-trajectory__provenance"
              role="group"
              aria-label={text('provenance')}
            >
              <button
                type="button"
                aria-pressed={provenance === 'all'}
                onClick={() => setProvenance('all')}
              >
                {text('all_sources')}
              </button>
              <button
                type="button"
                aria-pressed={provenance === 'main'}
                onClick={() =>
                  setProvenance(provenance === 'main' ? 'all' : 'main')
                }
              >
                {text('main')}
              </button>
              <button
                type="button"
                aria-pressed={provenance === 'subagent'}
                onClick={() =>
                  setProvenance(provenance === 'subagent' ? 'all' : 'subagent')
                }
              >
                {text('subagent')}
              </button>
            </span>
          </nav>
          {indexQuery.isLoading || pages.isLoading ? (
            <div className="provider-trajectory__loading" role="status">
              <Spin />
              {i18nText('agentFlow', 'trajectory.loading_first_page')}
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
          {!(indexQuery.isPending || pages.isPending) &&
          !(indexQuery.isError || pages.isError) &&
          !index.length ? (
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
            resetKey={`${logConversationId}:${generation}`}
            active={active && !invalidTime}
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
                  setRelatedTarget(null);
                  setRelatedStep(null);
                  virtualList.current?.focusKey(selectedStep.id);
                }}
              />
            </div>
            {relatedTarget && related.isFetching ? (
              <div role="status">
                <Spin />
                {i18nText('agentFlow', 'trajectory.loading_first_page')}
              </div>
            ) : null}
            {relatedTarget && related.isError ? (
              <Alert
                type="error"
                title={i18nText('agentFlow', 'auto.loading_failed')}
                action={
                  <Button onClick={() => void related.refetch()}>
                    {i18nText('agentFlow', 'auto.retry')}
                  </Button>
                }
              />
            ) : null}
            {relatedTarget &&
            related.isSuccess &&
            !related.data.items.length ? (
              <Alert type="warning" title={text('related_unloaded')} />
            ) : null}
            <ClientTrajectoryDetail
              key={`${generation}:${selectedStep.id}`}
              step={selectedStep}
              runId={logConversationId}
              loader={detailLoader}
              onRelated={(id) => {
                select(id);
              }}
            />
          </aside>
        ) : null}
      </div>
      <footer className="provider-trajectory__footer">
        <span role="status">
          {!(indexQuery.isError || pages.isError) &&
          (indexQuery.isFetching || pages.isFetching)
            ? i18nText('agentFlow', 'trajectory.loading_pages')
            : text('loaded')}
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
