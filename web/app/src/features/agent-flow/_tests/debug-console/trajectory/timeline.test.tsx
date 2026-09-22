import { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import {
  TrajectoryTimeline,
  inTrajectoryRange,
  type TrajectoryRange
} from '../../../components/debug-console/trajectory/TrajectoryTimeline';

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
