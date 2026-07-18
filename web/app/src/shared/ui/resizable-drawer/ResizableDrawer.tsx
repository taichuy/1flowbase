import { Drawer } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';

import './resizable-drawer.css';

export interface ResizableDrawerProps {
  open: boolean;
  title: ReactNode;
  children: ReactNode;
  onClose: () => void;
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  resizeLabel: string;
  extra?: ReactNode;
  footer?: ReactNode;
  destroyOnClose?: boolean;
  rootClassName?: string;
  bodyClassName?: string;
}

const DEFAULT_WIDTH = 720;
const DEFAULT_MIN_WIDTH = 480;
const DEFAULT_MAX_WIDTH = 1200;
let drawerInstanceSeed = 0;

export function ResizableDrawer({
  bodyClassName,
  children,
  defaultWidth = DEFAULT_WIDTH,
  destroyOnClose,
  extra,
  footer,
  maxWidth = DEFAULT_MAX_WIDTH,
  minWidth = DEFAULT_MIN_WIDTH,
  onClose,
  open,
  resizeLabel,
  rootClassName,
  title
}: ResizableDrawerProps) {
  const instanceClassNameRef = useRef<string | null>(null);
  if (instanceClassNameRef.current === null) {
    drawerInstanceSeed += 1;
    instanceClassNameRef.current = `resizable-drawer-instance-${drawerInstanceSeed}`;
  }

  const initialWidth = clampWidth(defaultWidth, minWidth, maxWidth);
  const [width, setWidth] = useState(initialWidth);
  const dragStartRef = useRef<{ pointerX: number; width: number } | null>(null);
  const liveWidthRef = useRef(initialWidth);
  const pendingWidthRef = useRef<number | null>(null);
  const animationFrameRef = useRef<number | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const nextWidth = clampWidth(defaultWidth, minWidth, maxWidth);
    liveWidthRef.current = nextWidth;
    setWidth(nextWidth);
  }, [defaultWidth, maxWidth, minWidth, open]);

  useEffect(() => {
    liveWidthRef.current = width;
  }, [width]);

  useEffect(() => {
    const drawerRootSelector = `.${instanceClassNameRef.current}`;
    const applyLiveWidth = (nextWidth: number) => {
      liveWidthRef.current = nextWidth;
      const drawerWrapper = document.querySelector<HTMLElement>(
        `${drawerRootSelector} .ant-drawer-content-wrapper`
      );
      if (drawerWrapper) {
        drawerWrapper.style.width = `${nextWidth}px`;
      }
    };

    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = dragStartRef.current;
      if (!dragStart) {
        return;
      }

      pendingWidthRef.current = clampWidth(
        dragStart.width + dragStart.pointerX - event.clientX,
        minWidth,
        maxWidth
      );
      if (animationFrameRef.current !== null) {
        return;
      }

      animationFrameRef.current = window.requestAnimationFrame(() => {
        animationFrameRef.current = null;
        const nextWidth = pendingWidthRef.current;
        pendingWidthRef.current = null;
        if (nextWidth !== null) {
          applyLiveWidth(nextWidth);
        }
      });
    };

    const handleMouseUp = () => {
      const pendingWidth = pendingWidthRef.current;
      if (animationFrameRef.current !== null) {
        window.cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
      if (pendingWidth !== null) {
        pendingWidthRef.current = null;
        applyLiveWidth(pendingWidth);
      }
      setWidth(liveWidthRef.current);
      dragStartRef.current = null;
      document.body.classList.remove('resizable-drawer--resizing');
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      if (animationFrameRef.current !== null) {
        window.cancelAnimationFrame(animationFrameRef.current);
      }
      document.body.classList.remove('resizable-drawer--resizing');
    };
  }, [maxWidth, minWidth]);

  const resolvedRootClassName = [
    'resizable-drawer',
    instanceClassNameRef.current,
    rootClassName
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <Drawer
      destroyOnClose={destroyOnClose}
      extra={extra}
      footer={footer}
      open={open}
      placement="right"
      rootClassName={resolvedRootClassName}
      title={title}
      width={width}
      onClose={onClose}
    >
      <div
        className={['resizable-drawer__body', bodyClassName]
          .filter(Boolean)
          .join(' ')}
      >
        <div
          aria-label={resizeLabel}
          aria-orientation="vertical"
          aria-valuemax={maxWidth}
          aria-valuemin={minWidth}
          aria-valuenow={width}
          className="resizable-drawer__resize-handle"
          role="separator"
          tabIndex={0}
          onKeyDown={(event) => {
            if (event.key === 'ArrowLeft') {
              event.preventDefault();
              setWidth((current) => clampWidth(current + 40, minWidth, maxWidth));
            } else if (event.key === 'ArrowRight') {
              event.preventDefault();
              setWidth((current) => clampWidth(current - 40, minWidth, maxWidth));
            } else if (event.key === 'Home') {
              event.preventDefault();
              setWidth(minWidth);
            } else if (event.key === 'End') {
              event.preventDefault();
              setWidth(maxWidth);
            }
          }}
          onMouseDown={(event) => {
            event.preventDefault();
            dragStartRef.current = {
              pointerX: event.clientX,
              width: liveWidthRef.current
            };
            document.body.classList.add('resizable-drawer--resizing');
          }}
        />
        {children}
      </div>
    </Drawer>
  );
}

function clampWidth(width: number, minWidth: number, maxWidth: number) {
  return Math.min(maxWidth, Math.max(minWidth, width));
}
