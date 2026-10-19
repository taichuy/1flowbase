import './navigation';
import { Profiler, useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { TrajectoryTimeline } from '../../../components/debug-console/trajectory/TrajectoryTimeline';
import { inTrajectoryRange, type TrajectoryRange } from '../../../components/debug-console/trajectory/timeline-model';

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(60);
});

test('brush range filters inclusively, handles adjust it, reset restores newly loaded domain', () => {
  const select = vi.fn();
  function Fixture({ end }: { end: number }) {
    const [range, setRange] = useState<TrajectoryRange>(null);
    return (
      <>
        <TrajectoryTimeline
          points={[0, 50, end].map((value) => ({
            id: String(value),
            value,
            lane: 'model',
            label: `point ${value}`
          }))}
          range={range}
          onChange={setRange}
          onSelect={select}
          selected={null}
          timeScale={false}
        />
        <output data-testid="visible">
          {[0, 50, end]
            .filter((value) => inTrajectoryRange(value, range))
            .join(',')}
        </output>
      </>
    );
  }
  const { rerender } = render(<Fixture end={100} />);
  const track = screen.getByTestId('trajectory-brush');
  vi.spyOn(track, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    width: 100
  } as DOMRect);
  track.setPointerCapture = vi.fn();
  // jsdom PointerEvent uses the MouseEvent coordinates for the real drag handlers.
  vi.stubGlobal('PointerEvent', MouseEvent);
  fireEvent.pointerDown(track, { button: 0, clientX: 25 });
  fireEvent.pointerMove(track, { clientX: 75 });
  fireEvent.pointerUp(track, { clientX: 75 });
  expect(screen.getByTestId('visible')).toHaveTextContent(/^50$/);
  const handles = screen.getAllByRole('slider');
  fireEvent.keyDown(handles[0], { key: 'Home' });
  expect(screen.getByTestId('visible')).toHaveTextContent(/^0,50$/);
  rerender(<Fixture end={200} />);
  expect(screen.getByTestId('visible')).toHaveTextContent(/^0,50$/);
  fireEvent.doubleClick(track);
  expect(screen.getByTestId('visible')).toHaveTextContent(/^0,50,200$/);
  fireEvent.click(screen.getByRole('button', { name: 'point 200' }));
  expect(select).toHaveBeenCalledWith('200');
  vi.unstubAllGlobals();
});

test('dense timeline bounds DOM while retaining exact distant and equal-time event selection', () => {
  const width = vi
    .spyOn(HTMLElement.prototype, 'clientWidth', 'get')
    .mockReturnValue(800);
  const select = vi.fn();
  const context = {
    scale: vi.fn(),
    fillRect: vi.fn(),
    fillStyle: '',
    globalAlpha: 1
  };
  const canvas = vi
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockReturnValue(context as unknown as CanvasRenderingContext2D);
  const points = Array.from({ length: 10000 }, (_, i) => ({
    id: String(i),
    value: Math.floor(i / 2),
    lane: 'model',
    label: `event ${i}`
  }));
  const commits: number[] = [];
  try {
    const { container } = render(
      <Profiler
        id="dense"
        onRender={() =>
          commits.push(
            document.querySelectorAll('.provider-trajectory__block').length
          )
        }
      >
        <TrajectoryTimeline
          points={points}
          range={null}
          onChange={() => undefined}
          onSelect={select}
          selected={null}
          timeScale={false}
        />
      </Profiler>
    );
    expect(
      container.querySelectorAll('.provider-trajectory__block').length
    ).toBeLessThanOrEqual(2);
    expect(container.querySelectorAll('canvas')).toHaveLength(1);
    const axis = screen.getByTestId('trajectory-brush');
    fireEvent.keyDown(axis, { key: 'End' });
    fireEvent.keyDown(axis, { key: 'Enter' });
    expect(select).toHaveBeenLastCalledWith('9999');
    fireEvent.keyDown(axis, { key: 'ArrowLeft' });
    fireEvent.keyDown(axis, { key: 'Enter' });
    expect(select).toHaveBeenLastCalledWith('9998');
    expect(Math.max(...commits)).toBeLessThanOrEqual(2);
    expect(context.fillRect.mock.calls.length).toBeGreaterThanOrEqual(10000);
    expect(context.fillRect.mock.calls.length % 10000).toBe(0);
  } finally {
    width.mockRestore();
    canvas.mockRestore();
  }
});

test('a populated zero-width or hidden timeline never mounts every event', () => {
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(0);
  const points = Array.from({ length: 10000 }, (_, i) => ({
    id: String(i),
    value: i,
    lane: 'model',
    label: `hidden event ${i}`
  }));
  const { container } = render(
    <TrajectoryTimeline
      points={points}
      range={null}
      onChange={() => undefined}
      onSelect={() => undefined}
      selected={null}
      timeScale={false}
    />
  );
  expect(
    container.querySelectorAll('.provider-trajectory__block').length
  ).toBeLessThanOrEqual(2);
  expect(container.querySelectorAll('canvas')).toHaveLength(1);
});
