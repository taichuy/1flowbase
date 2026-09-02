import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import HolderOutlined from '@ant-design/icons/es/icons/HolderOutlined';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  type RefObject
} from 'react';
import { createPortal } from 'react-dom';
import { Typography } from 'antd';
import { i18nText } from '../../../../shared/i18n/text';

type FloatingPanelBounds = {
  left: number;
  top: number;
  width: number;
  height: number;
};

type FloatingPanelPosition = {
  left: number;
  top: number;
};

type FloatingPanelResizeEdge = 'left' | 'right';

type FloatingSettingsPanelProps = {
  open: boolean;
  title: string;
  closeLabel: string;
  triggerRef: RefObject<HTMLElement | null>;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
  defaultWidth?: number;
  minWidth?: number;
  initialHeight?: number;
  minHeight?: number;
  gap?: number;
  margin?: number;
  dragHandleTestId?: string;
  leftResizeHandleTestId?: string;
  rightResizeHandleTestId?: string;
  bottomResizeHandleTestId?: string;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
  onClose: () => void;
};

const DEFAULT_WIDTH = 320;
const DEFAULT_MIN_WIDTH = 320;
const DEFAULT_MIN_HEIGHT = 240;
const DEFAULT_GAP = 24;
const DEFAULT_MARGIN = 16;
const FALLBACK_HEIGHT = 360;
const FALLBACK_VISIBLE_DRAG_HANDLE_SIZE = 48;

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function resolveBounds(container: HTMLElement | null): FloatingPanelBounds {
  if (container) {
    const rect = container.getBoundingClientRect();

    return {
      left: rect.left,
      top: rect.top,
      width: rect.width || container.clientWidth,
      height: rect.height || container.clientHeight
    };
  }

  return {
    left: 0,
    top: 0,
    width: window.innerWidth,
    height: window.innerHeight
  };
}

function resolveInitialHeight(
  bounds: FloatingPanelBounds,
  initialHeight: number | undefined,
  minHeight: number,
  margin: number
) {
  const fallbackHeight =
    bounds.height > 0 ? Math.round(bounds.height / 2) : FALLBACK_HEIGHT;
  const preferredHeight = initialHeight ?? fallbackHeight;

  return clampHeight(
    preferredHeight,
    bounds,
    { left: margin, top: margin },
    minHeight,
    margin
  );
}

function clampHeight(
  height: number,
  bounds: FloatingPanelBounds,
  position: FloatingPanelPosition,
  minHeight: number,
  margin: number
) {
  if (bounds.height <= 0) {
    return Math.max(height, minHeight);
  }

  const maxHeight = Math.max(minHeight, bounds.height - position.top - margin);

  return clamp(height, minHeight, maxHeight);
}

function clampPosition(
  position: FloatingPanelPosition,
  bounds: FloatingPanelBounds,
  panelHeight: number,
  panelWidth: number,
  margin: number
) {
  const maxLeft = Math.max(margin, bounds.width - panelWidth - margin);
  const maxTop = Math.max(margin, bounds.height - panelHeight - margin);

  return {
    left: clamp(position.left, margin, maxLeft),
    top: clamp(position.top, margin, maxTop)
  };
}

function clampDraggedPosition(
  position: FloatingPanelPosition,
  bounds: FloatingPanelBounds,
  visibleDragHandleWidth: number,
  visibleDragHandleHeight: number,
  margin: number
) {
  const maxLeft = Math.max(
    margin,
    bounds.width - visibleDragHandleWidth - margin
  );
  const maxTop = Math.max(
    margin,
    bounds.height - visibleDragHandleHeight - margin
  );

  return {
    left: clamp(position.left, margin, maxLeft),
    top: clamp(position.top, margin, maxTop)
  };
}

function clampWidth(
  width: number,
  bounds: FloatingPanelBounds,
  position: FloatingPanelPosition,
  minWidth: number,
  margin: number
) {
  const maxWidth = Math.max(minWidth, bounds.width - position.left - margin);

  return clamp(width, minWidth, maxWidth);
}

function resolveInitialPosition({
  trigger,
  bounds,
  panelHeight,
  panelWidth,
  gap,
  margin
}: {
  trigger: HTMLElement | null;
  bounds: FloatingPanelBounds;
  panelHeight: number;
  panelWidth: number;
  gap: number;
  margin: number;
}) {
  if (!trigger) {
    return clampPosition(
      {
        left: bounds.width - panelWidth - margin,
        top: margin
      },
      bounds,
      panelHeight,
      panelWidth,
      margin
    );
  }

  const triggerRect = trigger.getBoundingClientRect();
  const nodeDetail = trigger.closest<HTMLElement>('.agent-flow-node-detail');
  const detailRect = nodeDetail?.getBoundingClientRect();
  const preferredLeft = detailRect
    ? detailRect.left - bounds.left - panelWidth - gap
    : triggerRect.left - bounds.left - panelWidth - gap;

  // 如果左侧有足够的空间容纳浮窗，则悬浮于抽屉左侧
  // 否则靠最左侧边界（margin）显示，以最大程度减少对详情抽屉的遮挡
  const targetLeft = preferredLeft >= margin ? preferredLeft : margin;

  return clampPosition(
    {
      left: targetLeft,
      top: triggerRect.top - bounds.top
    },
    bounds,
    panelHeight,
    panelWidth,
    margin
  );
}

export function FloatingSettingsPanel({
  open,
  title,
  closeLabel,
  triggerRef,
  children,
  footer,
  className,
  defaultWidth = DEFAULT_WIDTH,
  minWidth = DEFAULT_MIN_WIDTH,
  initialHeight,
  minHeight = DEFAULT_MIN_HEIGHT,
  gap = DEFAULT_GAP,
  margin = DEFAULT_MARGIN,
  dragHandleTestId,
  leftResizeHandleTestId,
  rightResizeHandleTestId,
  bottomResizeHandleTestId,
  onMouseEnter,
  onMouseLeave,
  onClose
}: FloatingSettingsPanelProps) {
  const [panelContainer, setPanelContainer] = useState<HTMLElement | null>(
    null
  );
  const [panelHeight, setPanelHeight] = useState(
    initialHeight ?? FALLBACK_HEIGHT
  );
  const [panelWidth, setPanelWidth] = useState(defaultWidth);
  const [panelPosition, setPanelPosition] = useState<FloatingPanelPosition>({
    left: margin,
    top: margin
  });
  const cleanupDragRef = useRef<(() => void) | null>(null);
  const titleId = useId();

  useEffect(() => {
    if (!open) {
      cleanupDragRef.current?.();
      return;
    }

    const nextContainer =
      triggerRef.current?.closest<HTMLElement>('.agent-flow-editor__body') ??
      null;
    const bounds = resolveBounds(nextContainer);
    const nextHeight = resolveInitialHeight(
      bounds,
      initialHeight,
      minHeight,
      margin
    );
    const nextWidth = clampWidth(
      defaultWidth,
      bounds,
      { left: margin, top: margin },
      minWidth,
      margin
    );

    setPanelContainer(nextContainer);
    setPanelHeight(nextHeight);
    setPanelWidth(nextWidth);
    setPanelPosition(
      resolveInitialPosition({
        trigger: triggerRef.current,
        bounds,
        panelHeight: nextHeight,
        panelWidth: nextWidth,
        gap,
        margin
      })
    );
  }, [
    defaultWidth,
    gap,
    initialHeight,
    margin,
    minHeight,
    minWidth,
    open,
    triggerRef
  ]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleEscape);
    return () => window.removeEventListener('keydown', handleEscape);
  }, [onClose, open]);

  useEffect(() => {
    if (!open) {
      return;
    }

    const handleResize = () => {
      const nextContainer =
        triggerRef.current?.closest<HTMLElement>('.agent-flow-editor__body') ??
        null;
      const bounds = resolveBounds(nextContainer);
      const nextHeight = resolveInitialHeight(
        bounds,
        initialHeight,
        minHeight,
        margin
      );

      setPanelContainer(nextContainer);
      setPanelHeight(nextHeight);
      setPanelPosition((current) => {
        const nextWidth = clampWidth(
          panelWidth,
          bounds,
          current,
          minWidth,
          margin
        );

        setPanelWidth(nextWidth);
        return clampPosition(current, bounds, nextHeight, nextWidth, margin);
      });
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [
    initialHeight,
    margin,
    minHeight,
    minWidth,
    open,
    panelWidth,
    triggerRef
  ]);

  useEffect(() => {
    return () => {
      cleanupDragRef.current?.();
    };
  }, []);

  function startDrag(event: ReactMouseEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();

    const bounds = resolveBounds(panelContainer);
    const dragHandleRect = event.currentTarget.getBoundingClientRect();
    const dragHandleWidth =
      dragHandleRect.width || FALLBACK_VISIBLE_DRAG_HANDLE_SIZE;
    const dragHandleHeight =
      dragHandleRect.height || FALLBACK_VISIBLE_DRAG_HANDLE_SIZE;
    const offsetX = event.clientX - bounds.left - panelPosition.left;
    const offsetY = event.clientY - bounds.top - panelPosition.top;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    cleanupDragRef.current?.();
    document.body.style.cursor = 'move';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      setPanelPosition(
        clampDraggedPosition(
          {
            left: moveEvent.clientX - bounds.left - offsetX,
            top: moveEvent.clientY - bounds.top - offsetY
          },
          bounds,
          dragHandleWidth,
          dragHandleHeight,
          margin
        )
      );
    };

    const cleanup = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', cleanup);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      cleanupDragRef.current = null;
    };

    cleanupDragRef.current = cleanup;
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', cleanup);
  }

  function startResize(
    event: ReactMouseEvent<HTMLDivElement>,
    edge: FloatingPanelResizeEdge
  ) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();

    const bounds = resolveBounds(panelContainer);
    const startX = event.clientX;
    const startWidth = panelWidth;
    const startLeft = panelPosition.left;
    const startRight = startLeft + startWidth;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    cleanupDragRef.current?.();
    document.body.style.cursor = 'ew-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (edge === 'right') {
        setPanelWidth(
          clampWidth(
            startWidth + moveEvent.clientX - startX,
            bounds,
            panelPosition,
            minWidth,
            margin
          )
        );
        return;
      }

      const maxWidth = Math.max(minWidth, startRight - margin);
      const nextWidth = clamp(
        startRight - (moveEvent.clientX - bounds.left),
        minWidth,
        maxWidth
      );

      setPanelWidth(nextWidth);
      setPanelPosition((current) => ({
        ...current,
        left: startRight - nextWidth
      }));
    };

    const cleanup = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', cleanup);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      cleanupDragRef.current = null;
    };

    cleanupDragRef.current = cleanup;
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', cleanup);
  }

  function startBottomResize(event: ReactMouseEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();

    const bounds = resolveBounds(panelContainer);
    const startY = event.clientY;
    const startHeight = panelHeight;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    cleanupDragRef.current?.();
    document.body.style.cursor = 'ns-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      setPanelHeight(
        clampHeight(
          startHeight + moveEvent.clientY - startY,
          bounds,
          panelPosition,
          minHeight,
          margin
        )
      );
    };

    const cleanup = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', cleanup);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      cleanupDragRef.current = null;
    };

    cleanupDragRef.current = cleanup;
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', cleanup);
  }

  if (!open) {
    return null;
  }

  const panel = (
    <div
      aria-labelledby={titleId}
      aria-modal="false"
      className={['agent-flow-model-settings__panel', className]
        .filter(Boolean)
        .join(' ')}
      role="dialog"
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      style={{
        position: panelContainer ? 'absolute' : 'fixed',
        width: `${panelWidth}px`,
        height: `${panelHeight}px`,
        left: `${panelPosition.left}px`,
        top: `${panelPosition.top}px`
      }}
    >
      <div className="agent-flow-model-settings__panel-header">
        <div
          className="agent-flow-model-settings__drag-handle"
          data-testid={dragHandleTestId}
          onMouseDown={startDrag}
        >
          <HolderOutlined
            aria-hidden="true"
            className="agent-flow-model-settings__drag-icon"
          />
          <Typography.Title
            id={titleId}
            level={4}
            className="agent-flow-model-settings__panel-title"
          >
            {title}
          </Typography.Title>
        </div>
        <button
          aria-label={closeLabel}
          className="agent-flow-model-settings__close"
          onClick={onClose}
          type="button"
        >
          <CloseOutlined />
        </button>
      </div>

      <div
        aria-label={i18nText('agentFlow', 'auto.adjust_width_left', {
          value1: title
        })}
        aria-orientation="vertical"
        className="agent-flow-model-settings__resize-handle agent-flow-model-settings__resize-handle--left"
        data-testid={leftResizeHandleTestId}
        onMouseDown={(event) => startResize(event, 'left')}
        role="separator"
      />

      <div
        aria-label={i18nText('agentFlow', 'auto.adjust_width_right', {
          value1: title
        })}
        aria-orientation="vertical"
        className="agent-flow-model-settings__resize-handle agent-flow-model-settings__resize-handle--right"
        data-testid={rightResizeHandleTestId}
        onMouseDown={(event) => startResize(event, 'right')}
        role="separator"
      />

      <div
        aria-label={i18nText('agentFlow', 'auto.adjust_height_downwards', {
          value1: title
        })}
        aria-orientation="horizontal"
        className="agent-flow-model-settings__resize-handle agent-flow-model-settings__resize-handle--bottom"
        data-testid={bottomResizeHandleTestId}
        onMouseDown={startBottomResize}
        role="separator"
      />

      <div className="agent-flow-model-settings__panel-body">{children}</div>
      {footer}
    </div>
  );

  return createPortal(panel, panelContainer ?? document.body);
}
