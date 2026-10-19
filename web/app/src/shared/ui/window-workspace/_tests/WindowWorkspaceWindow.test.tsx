import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { WindowWorkspaceWindow } from '../WindowWorkspaceWindow';

describe('WindowWorkspaceWindow keyboard resize', () => {
  test('focusable edges resize and persist through the same geometry owner', () => {
    const onActivate = vi.fn();
    const onInteractionEnd = vi.fn();
    render(
      <WindowWorkspaceWindow
        active
        dragHandleSelector=".header"
        initialRect={() => ({ left: 100, top: 100, width: 500, height: 350 })}
        minWidth={300}
        minHeight={200}
        onActivate={onActivate}
        onInteractionEnd={onInteractionEnd}
        resizeLabel={(edge) => `Resize ${edge}`}
        testId="window"
        title="Keyboard window"
      >
        Content
      </WindowWorkspaceWindow>
    );
    const right = screen.getByRole('separator', { name: 'Resize right' });
    expect(right).toHaveAttribute('tabindex', '0');
    expect(right).toHaveAttribute('aria-orientation', 'vertical');
    expect(
      screen.getByRole('separator', { name: 'Resize left' })
    ).toHaveAttribute('aria-orientation', 'vertical');
    expect(
      screen.getByRole('separator', { name: 'Resize top' })
    ).toHaveAttribute('aria-orientation', 'horizontal');
    expect(
      screen.getByRole('separator', { name: 'Resize bottom' })
    ).toHaveAttribute('aria-orientation', 'horizontal');
    right.focus();
    expect(right).toHaveFocus();
    fireEvent.keyDown(right, { key: 'ArrowRight' });
    expect(screen.getByTestId('window')).toHaveStyle({ width: '510px' });
    expect(onInteractionEnd).toHaveBeenLastCalledWith({
      left: 100,
      top: 100,
      width: 510,
      height: 350
    });
    fireEvent.keyDown(screen.getByRole('separator', { name: 'Resize top' }), {
      key: 'ArrowDown'
    });
    expect(onInteractionEnd).toHaveBeenLastCalledWith({
      left: 100,
      top: 110,
      width: 510,
      height: 340
    });
    expect(onActivate).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(right, { key: 'ArrowDown' });
    expect(onActivate).toHaveBeenCalledTimes(2);
    for (let step = 0; step < 30; step += 1) {
      fireEvent.keyDown(right, { key: 'ArrowLeft' });
    }
    expect(right).toHaveAttribute('aria-valuenow', '300');
    expect(screen.getByTestId('window')).toHaveStyle({ width: '300px' });
  });
});
