import { useMemo, useRef, type CSSProperties, type PointerEvent } from 'react';
import { Button, Tooltip } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';

export type TrajectoryRange = readonly [number, number] | null;
export type TimelinePoint = {
  id: string;
  value: number;
  lane: string;
  label: string;
};
export function inTrajectoryRange(value: number, range: TrajectoryRange) {
  return !range || (value >= range[0] && value <= range[1]);
}

/** Full loaded domain stays fixed while the brush controls the ledger viewport. */
export function TrajectoryTimeline({
  points,
  range,
  onChange,
  onSelect,
  selected,
  timeScale = true
}: {
  points: TimelinePoint[];
  range: TrajectoryRange;
  onChange: (range: TrajectoryRange) => void;
  onSelect: (id: string) => void;
  selected: string | null;
  timeScale?: boolean;
}) {
  const track = useRef<HTMLDivElement>(null);
  const drag = useRef<{
    x: number;
    value: number;
    mode: string;
    range: readonly [number, number];
    moved: boolean;
  } | null>(null);
  const domain = useMemo(() => {
    const values = points.map((p) => p.value).filter(Number.isFinite);
    return values.reduce(
      ([min, max], value) => [Math.min(min, value), Math.max(max, value)],
      [values[0] ?? 0, values[0] ?? 0]
    );
  }, [points]);
  const [min, max] = domain;
  const span = Math.max(1, max - min);
  const position = (value: number) =>
    Math.max(0, Math.min(100, ((value - min) / span) * 100));
  const valueAt = (event: PointerEvent) => {
    const rect = track.current!.getBoundingClientRect();
    return (
      min +
      Math.max(
        0,
        Math.min(1, (event.clientX - rect.left) / Math.max(1, rect.width))
      ) *
        span
    );
  };
  function move(event: PointerEvent) {
    const gesture = drag.current;
    if (!gesture) return;
    if (Math.abs(event.clientX - gesture.x) < 3 && !gesture.moved) return;
    gesture.moved = true;
    const value = valueAt(event);
    if (gesture.mode === 'move') {
      const width = gesture.range[1] - gesture.range[0];
      const start = Math.max(
        min,
        Math.min(max - width, gesture.range[0] + value - gesture.value)
      );
      onChange([start, start + width]);
    } else {
      const anchor =
        gesture.mode === 'start'
          ? gesture.range[1]
          : gesture.mode === 'end'
            ? gesture.range[0]
            : gesture.value;
      onChange([Math.min(anchor, value), Math.max(anchor, value)]);
    }
  }
  const format = (value: number) =>
    timeScale ? formatDateTime(new Date(value)) : `#${Math.round(value) + 1}`;
  return (
    <section
      className="trajectory-timeline"
      aria-label={i18nText('agentFlow', 'trajectory.timeline')}
    >
      <div className="provider-trajectory__timeline">
        <div className="provider-trajectory__lane-labels">
          <span>{i18nText('agentFlow', 'auto.input')}</span>
          <span>{i18nText('agentFlow', 'auto.model')}</span>
          <span>{i18nText('agentFlow', 'auto.tools')}</span>
        </div>
        <div
          ref={track}
          className="provider-trajectory__lanes"
          data-testid="trajectory-brush"
          onPointerDown={(event) => {
            if (event.button !== 0 || !points.length) return;
            const mode = (event.target as HTMLElement).dataset.brush ?? 'new';
            drag.current = {
              x: event.clientX,
              value: valueAt(event),
              mode,
              range: range ?? [min, max],
              moved: false
            };
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={move}
          onPointerUp={(event) => {
            if (
              drag.current &&
              !drag.current.moved &&
              drag.current.mode === 'new'
            ) {
              const value = valueAt(event);
              const nearest = points.reduce((a, b) =>
                Math.abs(a.value - value) <= Math.abs(b.value - value) ? a : b
              );
              onChange(null);
              onSelect(nearest.id);
            }
            drag.current = null;
          }}
          onPointerCancel={() => {
            drag.current = null;
          }}
          onDoubleClick={() => onChange(null)}
        >
          {range ? (
            <div
              className="trajectory-timeline__selection"
              data-brush="move"
              style={{
                left: `${position(range[0])}%`,
                width: `${position(range[1]) - position(range[0])}%`
              }}
            />
          ) : null}
          {points.map((point) => (
            <Tooltip
              key={point.id}
              title={`${point.label} · ${format(point.value)}`}
            >
              <button
                type="button"
                className="provider-trajectory__block"
                data-lane={point.lane}
                data-selected={selected === point.id || undefined}
                data-dimmed={
                  !inTrajectoryRange(point.value, range) || undefined
                }
                aria-label={point.label}
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => {
                  if (!inTrajectoryRange(point.value, range)) onChange(null);
                  onSelect(point.id);
                }}
                style={
                  {
                    '--trajectory-x': `${position(point.value)}%`,
                    '--trajectory-width': `${Math.min(2, 65 / Math.max(points.length, 1))}%`
                  } as CSSProperties
                }
              />
            </Tooltip>
          ))}
          {range
            ? (['start', 'end'] as const).map((edge, index) => (
                <div
                  key={edge}
                  role="slider"
                  tabIndex={0}
                  data-brush={edge}
                  className="trajectory-timeline__handle"
                  style={{ left: `${position(range[index])}%` }}
                  aria-label={i18nText(
                    'agentFlow',
                    edge === 'start'
                      ? 'trajectory.range_start'
                      : 'trajectory.range_end'
                  )}
                  aria-valuemin={min}
                  aria-valuemax={max}
                  aria-valuenow={range[index]}
                  aria-valuetext={format(range[index])}
                  onKeyDown={(event) => {
                    if (event.key === 'Escape') {
                      onChange(null);
                      return;
                    }
                    if (
                      !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(
                        event.key
                      )
                    )
                      return;
                    event.preventDefault();
                    const value =
                      event.key === 'Home'
                        ? min
                        : event.key === 'End'
                          ? max
                          : range[index] +
                            (span / 100) * (event.key === 'ArrowLeft' ? -1 : 1);
                    onChange(
                      index === 0
                        ? [Math.max(min, Math.min(range[1], value)), range[1]]
                        : [range[0], Math.min(max, Math.max(range[0], value))]
                    );
                  }}
                />
              ))
            : null}
        </div>
      </div>
      <div className="trajectory-timeline__caption">
        <span>
          {points.length
            ? `${format(range?.[0] ?? min)} — ${format(range?.[1] ?? max)}`
            : '—'}
        </span>
        <Button
          size="small"
          type="text"
          disabled={!range}
          onClick={() => onChange(null)}
        >
          {i18nText('agentFlow', 'trajectory.range_all')}
        </Button>
      </div>
    </section>
  );
}
