import { useMemo, useRef, useState, type CSSProperties } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Input, Spin, Tooltip } from 'antd';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import type { ClientTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../../shared/i18n/format';
import { ClientTrajectoryDetail } from './ClientTrajectoryDetail';
import { categoryLabel, clientLane } from './presentation';
import { integrityLabel } from '../trajectory-presentation';
import './client-trajectory.css';

export function ClientTrajectoryWorkspace({
  runId,
  nodeRunId,
  loader
}: {
  runId: string;
  nodeRunId?: string;
  loader: ConversationLogTraceLoader;
}) {
  const [scope, setScope] = useState(nodeRunId);
  const [selected, setSelected] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(new Set<string>());
  const [detailWidth, setDetailWidth] = useState<number | null>(null);
  const split = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const rows = useRef(new Map<string, HTMLButtonElement>());
  const pages = useInfiniteQuery({
    queryKey: ['client-trajectory', runId, scope ?? 'run'],
    enabled: Boolean(loader.loadClientTrajectory),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadClientTrajectory!(runId, scope, pageParam),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchOnWindowFocus: false
  });
  const items = useMemo(
    () => pages.data?.pages.flatMap((page) => page.items) ?? [],
    [pages.data]
  );
  const selectedStep = items.find((step) => step.id === selected);
  const categories = [...new Set(items.map((step) => step.category))];
  const query = search.trim().toLocaleLowerCase();
  const matches = (step: ClientTrajectoryStep) =>
    (!category || step.category === category) &&
    (!query ||
      [
        step.name,
        step.preview,
        step.call_id,
        step.parameters_preview,
        step.result_preview
      ]
        .join(' ')
        .toLocaleLowerCase()
        .includes(query));
  // Indexing backend parent identities is presentation only; protocol classification lives on the server.
  const requests = useMemo(() => {
    const groups = new Map<string, ClientTrajectoryStep[]>();
    for (const step of items) {
      const group = groups.get(step.request_id) ?? [];
      group.push(step);
      groups.set(step.request_id, group);
    }
    return [...groups.entries()];
  }, [items]);
  function focus(id: string) {
    const step = items.find((item) => item.id === id);
    if (!step) return;
    setSelected(id);
    setCategory(null);
    setSearch('');
    setCollapsed((current) => {
      const next = new Set(current);
      next.delete(step.request_id);
      next.delete(`${step.request_id}:${step.category}`);
      return next;
    });
    requestAnimationFrame(() =>
      rows.current.get(id)?.scrollIntoView?.({ block: 'nearest' })
    );
  }
  function toggle(id: string) {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  function resize(width: number) {
    setDetailWidth(
      Math.max(
        320,
        Math.min(720, (split.current?.clientWidth ?? 1000) - 280, width)
      )
    );
  }
  function renderStep(step: ClientTrajectoryStep) {
    return (
      <button
        key={step.id}
        type="button"
        ref={(element) => {
          if (element) rows.current.set(step.id, element);
          else rows.current.delete(step.id);
        }}
        className="provider-trajectory__row client-trajectory__row"
        data-lane={clientLane(step)}
        data-selected={selected === step.id || undefined}
        aria-label={`${categoryLabel(step.category)} · ${step.name}`}
        aria-pressed={selected === step.id}
        onClick={() => setSelected(step.id)}
      >
        <span className="provider-trajectory__marker" aria-hidden="true" />
        <span className="provider-trajectory__kind">
          {categoryLabel(step.category)}
        </span>
        <span className="client-trajectory__summary">
          <strong>{step.name}</strong>
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
  }
  return (
    <div className="provider-trajectory client-trajectory">
      <div className="provider-trajectory__toolbar">
        <div className="provider-trajectory__controls">
          <span className="client-trajectory__protocol">Responses</span>
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
            {i18nText('agentFlow', 'clientTrajectory.expand')}
          </Button>
          <Button
            size="small"
            type="text"
            onClick={() => setCollapsed(new Set(requests.map(([id]) => id)))}
          >
            {i18nText('agentFlow', 'clientTrajectory.collapse')}
          </Button>
          <Button
            size="small"
            type="text"
            loading={pages.isRefetching}
            onClick={() => void pages.refetch()}
          >
            {i18nText('agentFlow', 'clientTrajectory.refresh')}
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
      <div
        className="provider-trajectory__timeline"
        aria-label={i18nText('agentFlow', 'trajectory.timeline')}
      >
        <div className="provider-trajectory__lane-labels">
          <span>{i18nText('agentFlow', 'auto.input')}</span>
          <span>{i18nText('agentFlow', 'auto.model')}</span>
          <span>{i18nText('agentFlow', 'auto.tools')}</span>
        </div>
        <div className="provider-trajectory__lanes">
          {items.map((step, index) => (
            <Tooltip
              key={step.id}
              title={`${categoryLabel(step.category)} · ${step.preview}`}
            >
              <button
                type="button"
                className="provider-trajectory__block"
                data-lane={clientLane(step)}
                data-selected={selected === step.id || undefined}
                data-dimmed={!matches(step) || undefined}
                aria-label={`${categoryLabel(step.category)} #${step.sequence}`}
                onClick={() => focus(step.id)}
                style={
                  {
                    '--trajectory-x': `${(index / Math.max(items.length, 1)) * 98}%`,
                    '--trajectory-width': `${Math.min(3, 72 / Math.max(items.length, 1))}%`
                  } as CSSProperties
                }
              />
            </Tooltip>
          ))}
        </div>
      </div>
      <div className="provider-trajectory__split" ref={split}>
        <div className="provider-trajectory__ledger">
          <nav
            className="client-trajectory__categories"
            aria-label={i18nText('agentFlow', 'clientTrajectory.categories')}
          >
            <button
              type="button"
              aria-pressed={!category}
              onClick={() => setCategory(null)}
            >
              {i18nText('agentFlow', 'clientTrajectory.all')}
            </button>
            {categories.map((key) => (
              <button
                type="button"
                key={key}
                aria-pressed={category === key}
                onClick={() => setCategory(category === key ? null : key)}
              >
                {categoryLabel(key)}{' '}
                <span>
                  {items.filter((step) => step.category === key).length}
                </span>
              </button>
            ))}
          </nav>
          {scope ? (
            <div className="client-trajectory__note">
              {i18nText('agentFlow', 'clientTrajectory.node_scope')}{' '}
              <Button
                type="link"
                size="small"
                onClick={() => {
                  setScope(undefined);
                  setSelected(null);
                }}
              >
                {i18nText('agentFlow', 'clientTrajectory.open_run')}
              </Button>
            </div>
          ) : null}
          {pages.isLoading ? (
            <div className="provider-trajectory__loading">
              <Spin />
            </div>
          ) : null}
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
          {!pages.isLoading && !pages.isError && !items.some(matches) ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText('agentFlow', 'clientTrajectory.no_records')}
            />
          ) : null}
          {requests.map(([id, steps]) => {
            const root = steps.find((step) => step.category === 'request');
            const children = steps.filter(
              (step) => step.category !== 'request' && matches(step)
            );
            if (!children.length && !(root && matches(root))) return null;
            const lanes = [...new Set(children.map((step) => step.category))];
            return (
              <section key={id} className="client-trajectory__request">
                <div className="client-trajectory__request-header">
                  <button
                    type="button"
                    className="client-trajectory__fold"
                    aria-label={i18nText(
                      'agentFlow',
                      'clientTrajectory.request'
                    )}
                    aria-expanded={!collapsed.has(id)}
                    onClick={() => toggle(id)}
                  >
                    {collapsed.has(id) ? <RightOutlined /> : <DownOutlined />}
                  </button>
                  <button
                    type="button"
                    className="client-trajectory__request-select"
                    onClick={() => root && setSelected(root.id)}
                  >
                    <strong>{categoryLabel('request')}</strong>
                    <span>{root?.preview || id}</span>
                    <small>{root ? formatDateTime(root.created_at) : ''}</small>
                  </button>
                </div>
                {!collapsed.has(id)
                  ? lanes.map((lane) => {
                      const key = `${id}:${lane}`;
                      const rows = children.filter(
                        (step) => step.category === lane
                      );
                      return (
                        <section
                          key={key}
                          className="client-trajectory__category-group"
                        >
                          <button
                            className="client-trajectory__category-header"
                            type="button"
                            aria-expanded={!collapsed.has(key)}
                            onClick={() => toggle(key)}
                          >
                            {collapsed.has(key) ? (
                              <RightOutlined />
                            ) : (
                              <DownOutlined />
                            )}
                            <span>{categoryLabel(lane)}</span>
                            <small>{rows.length}</small>
                          </button>
                          {!collapsed.has(key) ? rows.map(renderStep) : null}
                        </section>
                      );
                    })
                  : null}
              </section>
            );
          })}
          {pages.hasNextPage ? (
            <div className="provider-trajectory__more">
              <Button
                loading={pages.isFetchingNextPage}
                onClick={() => void pages.fetchNextPage()}
              >
                {i18nText('agentFlow', 'trajectory.more')}
              </Button>
            </div>
          ) : null}
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
                {selectedStep.name}
              </strong>
              <Button
                size="small"
                type="text"
                icon={<CloseOutlined />}
                aria-label={i18nText('agentFlow', 'clientTrajectory.close')}
                onClick={() => {
                  setSelected(null);
                  rows.current.get(selectedStep.id)?.focus();
                }}
              />
            </div>
            <ClientTrajectoryDetail
              key={selectedStep.id}
              step={selectedStep}
              loader={loader}
              nodeRunId={scope}
              onRelated={focus}
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
        <span>{i18nText('agentFlow', 'clientTrajectory.client')}</span>
      </footer>
    </div>
  );
}
