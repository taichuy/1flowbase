import type {
  CSSProperties,
  ReactNode,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent
} from 'react';

interface SideDockBaseProps {
  children: ReactNode;
  className?: string;
  'data-testid'?: string;
  isResizing?: boolean;
  style?: CSSProperties;
  width: number;
}

type AgentFlowSideDockProps = SideDockBaseProps & (
  | {
      resizable?: true;
      resizeLabel: string;
      minWidth: number;
      maxWidth: number;
      onResizeKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
      onResizeStart: (event: ReactMouseEvent<HTMLDivElement>) => void;
    }
  | {
      resizable: false;
      resizeLabel?: never;
      minWidth?: never;
      maxWidth?: never;
      onResizeKeyDown?: never;
      onResizeStart?: never;
    }
);

export function AgentFlowSideDock({
  children,
  className,
  'data-testid': dataTestId,
  isResizing = false,
  resizeLabel,
  resizable = true,
  style,
  width,
  minWidth,
  maxWidth,
  onResizeKeyDown,
  onResizeStart
}: AgentFlowSideDockProps) {
  return (
    <div
      className={className ?? 'agent-flow-editor__side-dock'}
      data-resizing={isResizing ? 'true' : 'false'}
      data-testid={dataTestId}
      style={{ ...style, width: `${width}px` }}
    >
      {resizable ? <div
        aria-label={resizeLabel}
        aria-orientation="vertical"
        className="agent-flow-editor__side-dock-resize-handle"
        onMouseDown={onResizeStart}
        onKeyDown={onResizeKeyDown}
        tabIndex={0}
        aria-valuenow={width}
        aria-valuemin={minWidth}
        aria-valuemax={maxWidth}
        role="separator"
      /> : null}
      {children}
    </div>
  );
}
