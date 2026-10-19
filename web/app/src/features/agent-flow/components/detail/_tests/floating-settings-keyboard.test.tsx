import { createRef } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { FloatingSettingsPanel } from '../FloatingSettingsPanel';

test('keyboard movement and edge resizing keep the same panel and report current dimensions', () => {
  render(
    <FloatingSettingsPanel
      open
      title="Settings"
      closeLabel="Close"
      triggerRef={createRef<HTMLElement>()}
      onClose={vi.fn()}
      dragHandleTestId="move"
      leftResizeHandleTestId="left"
      rightResizeHandleTestId="right"
      bottomResizeHandleTestId="bottom"
    >
      Content
    </FloatingSettingsPanel>
  );
  const panel = screen.getByRole('dialog');
  const move = screen.getByTestId('move');
  const left = screen.getByTestId('left');
  const right = screen.getByTestId('right');
  const bottom = screen.getByTestId('bottom');
  for (const handle of [move, left, right, bottom])
    expect(handle).toHaveAttribute('tabindex', '0');
  const initialLeft = Number.parseFloat(panel.style.left);
  fireEvent.keyDown(move, { key: 'ArrowLeft' });
  expect(Number.parseFloat(panel.style.left)).toBe(initialLeft - 16);
  const initialWidth = Number.parseFloat(panel.style.width);
  const fixedRight = Number.parseFloat(panel.style.left) + initialWidth;
  fireEvent.keyDown(left, { key: 'ArrowLeft' });
  expect(Number.parseFloat(panel.style.width)).toBe(initialWidth + 16);
  expect(
    Number.parseFloat(panel.style.left) + Number.parseFloat(panel.style.width)
  ).toBe(fixedRight);
  expect(left).toHaveAttribute('aria-valuenow', String(initialWidth + 16));
  fireEvent.keyDown(right, { key: 'ArrowLeft' });
  expect(Number.parseFloat(panel.style.width)).toBe(initialWidth);
  const initialHeight = Number.parseFloat(panel.style.height);
  fireEvent.keyDown(bottom, { key: 'ArrowDown' });
  expect(Number.parseFloat(panel.style.height)).toBe(initialHeight + 16);
  expect(bottom).toHaveAttribute('aria-valuenow', String(initialHeight + 16));
});
