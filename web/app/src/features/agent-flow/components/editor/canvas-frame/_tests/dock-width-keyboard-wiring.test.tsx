import type { KeyboardEvent } from 'react';
import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { useCanvasDockLayout } from '../use-canvas-dock-layout';
import {
  CONVERSATION_LOG_MIN_WIDTH,
  DEBUG_CONSOLE_MIN_WIDTH,
  HISTORY_DOCK_MIN_WIDTH,
  VARIABLES_DOCK_MIN_WIDTH,
  VARIABLE_CACHE_MIN_SIDEBAR_WIDTH
} from '../layout';

let layout: ReturnType<typeof useCanvasDockLayout>;
function Harness({
  selectedNodeId = null
}: {
  selectedNodeId?: string | null;
}) {
  layout = useCanvasDockLayout({
    selectedNodeId,
    historyOpen: true,
    nodeDetailWidth: 420,
    setPanelState: () => {},
    conversationLogMessage: null
  });
  return <div ref={layout.bodyRef} />;
}
const key = (value: string) =>
  ({
    key: value,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn()
  }) as unknown as KeyboardEvent<HTMLDivElement>;

beforeEach(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    }
  );
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 1600,
    bottom: 900,
    width: 1600,
    height: 900,
    toJSON() {
      return {};
    }
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const docks = [
  {
    label: 'debug',
    handler: 'handleDebugConsoleResizeKeyDown',
    width: 'boundedDebugConsoleWidth',
    max: 'maxDebugConsoleWidth',
    min: DEBUG_CONSOLE_MIN_WIDTH
  },
  {
    label: 'conversation log',
    handler: 'handleConversationLogResizeKeyDown',
    width: 'boundedConversationLogWidth',
    max: 'maxConversationLogWidth',
    min: CONVERSATION_LOG_MIN_WIDTH
  },
  {
    label: 'variables',
    handler: 'handleVariablesDockResizeKeyDown',
    width: 'boundedVariablesDockWidth',
    max: 'maxVariablesDockWidth',
    min: VARIABLES_DOCK_MIN_WIDTH
  },
  {
    label: 'history',
    handler: 'handleHistoryDockResizeKeyDown',
    width: 'boundedHistoryDockWidth',
    max: 'maxHistoryDockWidth',
    min: HISTORY_DOCK_MIN_WIDTH
  }
] as const;

test.each(docks)(
  '$label dock keyboard uses left to grow, right to shrink, and adaptive Home/End bounds',
  ({ handler, width, max, min }) => {
    const view = render(<Harness />);
    act(() => layout[handler](key('Home')));
    expect(layout[width]).toBe(min);
    act(() => layout[handler](key('ArrowLeft')));
    expect(layout[width]).toBe(min + 24);
    act(() => layout[handler](key('ArrowRight')));
    expect(layout[width]).toBe(min);
    act(() => layout[handler](key('End')));
    expect(layout[width]).toBe(layout[max]);
    const largerMax = layout[max];
    view.rerender(<Harness selectedNodeId="selected" />);
    expect(layout[max]).toBeLessThan(largerMax);
    act(() => layout[handler](key('End')));
    expect(layout[width]).toBe(layout[max]);
  }
);

test('variable dock keyboard updates only the active system, environment or conversation scope', () => {
  render(<Harness />);
  act(() => layout.setSystemVariablesOpen(true));
  act(() => layout.handleVariablesDockResizeKeyDown(key('Home')));
  act(() => {
    layout.setSystemVariablesOpen(false);
    layout.setEnvironmentVariablesOpen(true);
  });
  const environmentWidth = layout.boundedVariablesDockWidth;
  act(() => layout.handleVariablesDockResizeKeyDown(key('ArrowLeft')));
  act(() => {
    layout.setEnvironmentVariablesOpen(false);
    layout.setConversationVariablesOpen(true);
  });
  act(() => layout.handleVariablesDockResizeKeyDown(key('End')));
  act(() => {
    layout.setConversationVariablesOpen(false);
    layout.setEnvironmentVariablesOpen(true);
  });
  expect(layout.boundedVariablesDockWidth).toBe(environmentWidth + 24);
  act(() => {
    layout.setEnvironmentVariablesOpen(false);
    layout.setSystemVariablesOpen(true);
  });
  expect(layout.boundedVariablesDockWidth).toBe(VARIABLES_DOCK_MIN_WIDTH);
});

test('cache sidebar uses right to grow and left to shrink within its adaptive pane bounds', () => {
  const view = render(<Harness />);
  const initial = layout.boundedVariableCacheSidebarWidth;
  act(() => layout.handleVariableCacheSidebarResizeKeyDown(key('ArrowRight')));
  expect(layout.boundedVariableCacheSidebarWidth).toBe(initial + 24);
  act(() => layout.handleVariableCacheSidebarResizeKeyDown(key('ArrowLeft')));
  expect(layout.boundedVariableCacheSidebarWidth).toBe(initial);
  act(() => layout.handleVariableCacheSidebarResizeKeyDown(key('Home')));
  expect(layout.boundedVariableCacheSidebarWidth).toBe(
    VARIABLE_CACHE_MIN_SIDEBAR_WIDTH
  );
  act(() => layout.handleVariableCacheSidebarResizeKeyDown(key('End')));
  expect(layout.boundedVariableCacheSidebarWidth).toBe(
    layout.variableCacheSidebarMaxWidth
  );
  view.rerender(<Harness selectedNodeId="selected" />);
  act(() => layout.handleVariableCacheSidebarResizeKeyDown(key('End')));
  expect(layout.boundedVariableCacheSidebarWidth).toBe(
    layout.variableCacheSidebarMaxWidth
  );
});
