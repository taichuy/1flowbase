import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type ReactNode
} from 'react';

import {
  clampWindowWorkspaceRect,
  fitWindowWorkspaceRect,
  WINDOW_WORKSPACE_MIN_HEIGHT,
  WINDOW_WORKSPACE_MIN_WIDTH
} from './window-workspace-geometry';
import type { WindowWorkspaceRect } from './window-workspace-state';

import './window-workspace.css';

export type WindowWorkspaceResizeEdge = 'left' | 'right' | 'top' | 'bottom';

export interface WindowWorkspaceWindowProps {
  active: boolean;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
  dragHandleSelector: string;
  initialRect: () => WindowWorkspaceRect;
  minHeight?: number;
  minWidth?: number;
  onActivate: () => void;
  onRectChange?: (rect: WindowWorkspaceRect) => void;
  onInteractionEnd?: (rect: WindowWorkspaceRect) => void;
  rect?: WindowWorkspaceRect;
  resizeClassName?: (edge: WindowWorkspaceResizeEdge) => string | undefined;
  resizeEdges?: readonly WindowWorkspaceResizeEdge[];
  resizeLabel: (edge: WindowWorkspaceResizeEdge) => string;
  testId: string;
  title: string;
  zIndex?: number;
}

export function WindowWorkspaceWindow({
  active,
  bodyClassName,
  children,
  className,
  dragHandleSelector,
  initialRect,
  minHeight = WINDOW_WORKSPACE_MIN_HEIGHT,
  minWidth = WINDOW_WORKSPACE_MIN_WIDTH,
  onActivate,
  onInteractionEnd,
  onRectChange,
  rect,
  resizeClassName,
  resizeEdges = ['left', 'right', 'top', 'bottom'],
  resizeLabel,
  testId,
  title,
  zIndex
}: WindowWorkspaceWindowProps) {
  const [localRect, setLocalRect] = useState(() =>
    fitWindowWorkspaceRect(initialRect(), minWidth, minHeight)
  );
  const currentRect = rect ?? localRect;
  const currentRectRef = useRef(currentRect);
  currentRectRef.current = currentRect;
  const cleanupRef = useRef<(() => void) | null>(null);
  const commitRect = useCallback(
    (next: WindowWorkspaceRect) => {
      const current = currentRectRef.current;
      if (
        current.left === next.left &&
        current.top === next.top &&
        current.width === next.width &&
        current.height === next.height
      ) {
        return;
      }
      currentRectRef.current = next;
      if (onRectChange) onRectChange(next);
      else setLocalRect(next);
    },
    [onRectChange]
  );
  const setRect = useCallback(
    (next: WindowWorkspaceRect) => {
      commitRect(clampWindowWorkspaceRect(next, minWidth, minHeight));
    },
    [commitRect, minHeight, minWidth]
  );
  const fitRect = useCallback(
    (next: WindowWorkspaceRect) => {
      commitRect(fitWindowWorkspaceRect(next, minWidth, minHeight));
    },
    [commitRect, minHeight, minWidth]
  );

  useLayoutEffect(() => fitRect(currentRectRef.current), [fitRect]);
  useEffect(() => {
    const resize = () => fitRect(currentRectRef.current);
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, [fitRect]);
  useEffect(() => () => cleanupRef.current?.(), []);

  const begin = (
    event: ReactMouseEvent,
    cursor: string,
    move: (
      dx: number,
      dy: number,
      start: WindowWorkspaceRect
    ) => WindowWorkspaceRect
  ) => {
    if (event.button !== 0) return;
    event.preventDefault();
    onActivate();
    const startX = event.clientX;
    const startY = event.clientY;
    const start = currentRectRef.current;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    cleanupRef.current?.();
    document.body.style.cursor = cursor;
    document.body.style.userSelect = 'none';
    const mousemove = (next: MouseEvent) =>
      setRect(move(next.clientX - startX, next.clientY - startY, start));
    const cleanup = () => {
      window.removeEventListener('mousemove', mousemove);
      window.removeEventListener('mouseup', cleanup);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      cleanupRef.current = null;
      onInteractionEnd?.(currentRectRef.current);
    };
    cleanupRef.current = cleanup;
    window.addEventListener('mousemove', mousemove);
    window.addEventListener('mouseup', cleanup);
  };

  const startDrag = (event: ReactMouseEvent<HTMLDivElement>) => {
    const target = event.target instanceof HTMLElement ? event.target : null;
    if (
      !target?.closest(dragHandleSelector) ||
      target.closest(
        'button,a,input,textarea,select,[role="button"],[data-no-window-drag="true"]'
      )
    ) {
      onActivate();
      return;
    }
    begin(event, 'move', (dx, dy, start) => ({
      ...start,
      left: start.left + dx,
      top: start.top + dy
    }));
  };

  const resize = (edge: WindowWorkspaceResizeEdge, event: ReactMouseEvent) =>
    begin(
      event,
      edge === 'left' || edge === 'right' ? 'ew-resize' : 'ns-resize',
      (dx, dy, start) => {
        if (edge === 'left')
          return { ...start, left: start.left + dx, width: start.width - dx };
        if (edge === 'right') return { ...start, width: start.width + dx };
        if (edge === 'top')
          return { ...start, top: start.top + dy, height: start.height - dy };
        return { ...start, height: start.height + dy };
      }
    );

  const style: CSSProperties = {
    left: currentRect.left,
    top: currentRect.top,
    width: currentRect.width,
    height: currentRect.height,
    zIndex: zIndex ?? (active ? 1051 : 1050)
  };
  return (
    <div
      aria-label={title}
      aria-modal="false"
      className={['window-workspace-window', className]
        .filter(Boolean)
        .join(' ')}
      data-testid={testId}
      role="dialog"
      style={style}
      onMouseDownCapture={startDrag}
    >
      <div
        className={['window-workspace-window__body', bodyClassName]
          .filter(Boolean)
          .join(' ')}
      >
        {children}
      </div>
      {resizeEdges.map((edge) => (
        <div
          key={edge}
          aria-label={resizeLabel(edge)}
          aria-orientation={
            edge === 'left' || edge === 'right' ? 'vertical' : 'horizontal'
          }
          className={[
            'window-workspace-window__resize',
            `window-workspace-window__resize--${edge}`,
            resizeClassName?.(edge)
          ]
            .filter(Boolean)
            .join(' ')}
          role="separator"
          onMouseDown={(event) => resize(edge, event)}
        />
      ))}
    </div>
  );
}
