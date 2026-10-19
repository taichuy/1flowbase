import type { KeyboardEvent } from 'react';
import { act, renderHook } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { useCanvasDockLayout } from '../use-canvas-dock-layout';
import { VARIABLE_CACHE_MIN_HEIGHT } from '../layout';

test('variable cache keyboard resizing follows the top edge and clamps to the existing dock bounds', () => {
  const { result } = renderHook(() =>
    useCanvasDockLayout({
      selectedNodeId: null,
      historyOpen: false,
      nodeDetailWidth: 420,
      setPanelState: vi.fn(),
      conversationLogMessage: null
    })
  );
  const initialHeight = result.current.boundedVariableCacheHeight;
  const key = (value: string) =>
    ({
      key: value,
      preventDefault: vi.fn(),
      stopPropagation: vi.fn()
    }) as unknown as KeyboardEvent<HTMLDivElement>;
  act(() => result.current.handleVariableCacheResizeKeyDown(key('ArrowDown')));
  expect(result.current.boundedVariableCacheHeight).toBe(
    Math.max(VARIABLE_CACHE_MIN_HEIGHT, initialHeight - 16)
  );
  act(() => result.current.handleVariableCacheResizeKeyDown(key('ArrowUp')));
  expect(result.current.boundedVariableCacheHeight).toBe(initialHeight);
  for (let step = 0; step < 100; step += 1)
    act(() =>
      result.current.handleVariableCacheResizeKeyDown(key('ArrowDown'))
    );
  expect(result.current.boundedVariableCacheHeight).toBe(
    VARIABLE_CACHE_MIN_HEIGHT
  );
  for (let step = 0; step < 100; step += 1)
    act(() => result.current.handleVariableCacheResizeKeyDown(key('ArrowUp')));
  expect(result.current.boundedVariableCacheHeight).toBe(
    result.current.variableCacheMaxHeight
  );
});
