import { Drawer } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';

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
  zIndex?: number;
}

const DEFAULT_WIDTH = 720;
const DEFAULT_MIN_WIDTH = 480;
const DEFAULT_MAX_WIDTH = 1200;
const NATIVE_DRAGGER_HIT_WIDTH = 16;
const NATIVE_DRAGGER_OFFSET = -NATIVE_DRAGGER_HIT_WIDTH / 2;

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
  title,
  zIndex
}: ResizableDrawerProps) {
  const initialWidth = clampWidth(defaultWidth, minWidth, maxWidth);
  const [width, setWidth] = useState(initialWidth);

  useEffect(() => {
    if (!open) {
      return;
    }
    const nextWidth = clampWidth(defaultWidth, minWidth, maxWidth);
    setWidth(nextWidth);
  }, [defaultWidth, maxWidth, minWidth, open]);

  const resolvedRootClassName = ['resizable-drawer', rootClassName]
    .filter(Boolean)
    .join(' ');

  return (
    <Drawer
      defaultSize={initialWidth}
      destroyOnHidden={destroyOnClose}
      extra={extra}
      footer={footer}
      maxSize={maxWidth}
      open={open}
      placement="right"
      resizable={{
        onResize: (nextWidth) => {
          setWidth(clampWidth(nextWidth, minWidth, maxWidth));
        }
      }}
      rootClassName={resolvedRootClassName}
      size={width}
      styles={{
        dragger: {
          left: NATIVE_DRAGGER_OFFSET,
          width: NATIVE_DRAGGER_HIT_WIDTH
        },
        wrapper: {
          minWidth: `min(${minWidth}px, 100vw)`
        }
      }}
      title={title}
      zIndex={zIndex}
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
          className="resizable-drawer__keyboard-resize-handle"
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
        />
        {children}
      </div>
    </Drawer>
  );
}

function clampWidth(width: number, minWidth: number, maxWidth: number) {
  return Math.min(maxWidth, Math.max(minWidth, width));
}
