import { Drawer } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useId, useState } from 'react';

import './resizable-drawer.css';

export interface ResizableDrawerProps {
  open: boolean;
  title: ReactNode;
  ariaLabel?: string;
  children: ReactNode;
  onClose: () => void;
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  viewportGutter?: number;
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
const WIDTH_STORAGE_KEY_PREFIX = 'resizable-drawer:width:';

export function ResizableDrawer({
  ariaLabel,
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
  viewportGutter = 0,
  zIndex
}: ResizableDrawerProps) {
  const titleId = useId();
  const storageKey = pageWidthStorageKey();
  const initialWidth = resolveWidth(
    defaultWidth,
    minWidth,
    maxWidth,
    storageKey
  );
  const [width, setWidth] = useState(() => initialWidth);
  const viewportWidth =
    viewportGutter > 0 ? `calc(100vw - ${viewportGutter}px)` : '100vw';

  useEffect(() => {
    if (!open) {
      return;
    }
    const nextWidth = resolveWidth(
      defaultWidth,
      minWidth,
      maxWidth,
      storageKey
    );
    setWidth(nextWidth);
  }, [defaultWidth, maxWidth, minWidth, open, storageKey]);

  const updateWidth = (nextWidth: number) => {
    const resolvedWidth = clampWidth(nextWidth, minWidth, maxWidth);
    setWidth(resolvedWidth);
    writeStoredWidth(storageKey, resolvedWidth);
  };

  const resolvedRootClassName = ['resizable-drawer', rootClassName]
    .filter(Boolean)
    .join(' ');

  return (
    <Drawer
      aria-label={ariaLabel}
      aria-labelledby={titleId}
      defaultSize={initialWidth}
      destroyOnHidden={destroyOnClose}
      extra={extra}
      footer={footer}
      maxSize={maxWidth}
      open={open}
      placement="right"
      resizable={{
        onResize: (nextWidth) => {
          updateWidth(nextWidth);
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
          minWidth: `min(${minWidth}px, ${viewportWidth})`,
          ...(viewportGutter > 0 ? { maxWidth: viewportWidth } : {})
        }
      }}
      title={<span id={titleId}>{title}</span>}
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
              updateWidth(width + 40);
            } else if (event.key === 'ArrowRight') {
              event.preventDefault();
              updateWidth(width - 40);
            } else if (event.key === 'Home') {
              event.preventDefault();
              updateWidth(minWidth);
            } else if (event.key === 'End') {
              event.preventDefault();
              updateWidth(maxWidth);
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

function pageWidthStorageKey() {
  if (typeof window === 'undefined') {
    return null;
  }

  return `${WIDTH_STORAGE_KEY_PREFIX}${window.location.pathname}`;
}

function resolveWidth(
  defaultWidth: number,
  minWidth: number,
  maxWidth: number,
  storageKey: string | null
) {
  return clampWidth(
    readStoredWidth(storageKey) ?? defaultWidth,
    minWidth,
    maxWidth
  );
}

function readStoredWidth(storageKey: string | null) {
  if (!storageKey) {
    return null;
  }

  try {
    const storedWidth = window.localStorage.getItem(storageKey);

    if (!storedWidth?.trim()) {
      return null;
    }

    const width = Number(storedWidth);
    return Number.isFinite(width) ? width : null;
  } catch {
    return null;
  }
}

function writeStoredWidth(storageKey: string | null, width: number) {
  if (!storageKey) {
    return;
  }

  try {
    window.localStorage.setItem(storageKey, String(width));
  } catch {
    // Browser privacy settings must not disable local resizing.
  }
}
