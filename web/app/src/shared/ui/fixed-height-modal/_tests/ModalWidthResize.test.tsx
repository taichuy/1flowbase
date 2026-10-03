import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { ModalWidthResize } from '../ModalWidthResize';

test('both edges resize around the center and clamp to viewport bounds', () => {
  const onWidthChange = vi.fn();
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 700
  } as DOMRect);
  render(
    <ModalWidthResize onWidthChange={onWidthChange}>
      <div>Content</div>
    </ModalWidthResize>
  );
  const left = screen.getByRole('separator', {
    name: 'Resize dialog from left'
  });
  const right = screen.getByRole('separator', {
    name: 'Resize dialog from right'
  });
  fireEvent.keyDown(left, { key: 'ArrowLeft' });
  expect(onWidthChange).toHaveBeenLastCalledWith(732);
  fireEvent.keyDown(right, { key: 'ArrowLeft' });
  expect(onWidthChange).toHaveBeenLastCalledWith(668);
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 5000
  } as DOMRect);
  fireEvent.keyDown(right, { key: 'ArrowRight' });
  expect(onWidthChange).toHaveBeenLastCalledWith(window.innerWidth - 32);
  vi.restoreAllMocks();
});
