import { useEffect, useRef, useState, type ReactNode } from 'react';

import { Drawer } from 'antd';

const DEFAULT_DETAIL_DRAWER_WIDTH = 980;
const MIN_DETAIL_DRAWER_WIDTH = 720;
const MAX_DETAIL_DRAWER_WIDTH = 1280;
const KEYBOARD_RESIZE_STEP = 40;

function clampDetailDrawerWidth(width: number) {
  return Math.min(
    MAX_DETAIL_DRAWER_WIDTH,
    Math.max(MIN_DETAIL_DRAWER_WIDTH, width)
  );
}

export function DataModelDetailDrawer({
  children,
  open,
  title,
  onClose
}: {
  children: ReactNode;
  open: boolean;
  title: ReactNode;
  onClose: () => void;
}) {
  const [width, setWidth] = useState(DEFAULT_DETAIL_DRAWER_WIDTH);
  const dragStartRef = useRef<{ pointerX: number; width: number } | null>(null);

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = dragStartRef.current;
      if (!dragStart) {
        return;
      }

      setWidth(
        clampDetailDrawerWidth(
          dragStart.width + dragStart.pointerX - event.clientX
        )
      );
    };

    const handleMouseUp = () => {
      dragStartRef.current = null;
      document.body.classList.remove('data-model-panel--resizing-drawer');
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.classList.remove('data-model-panel--resizing-drawer');
    };
  }, []);

  const startResize = (event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    dragStartRef.current = {
      pointerX: event.clientX,
      width
    };
    document.body.classList.add('data-model-panel--resizing-drawer');
  };

  const handleResizeKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      setWidth((currentWidth) =>
        clampDetailDrawerWidth(currentWidth + KEYBOARD_RESIZE_STEP)
      );
      return;
    }

    if (event.key === 'ArrowRight') {
      event.preventDefault();
      setWidth((currentWidth) =>
        clampDetailDrawerWidth(currentWidth - KEYBOARD_RESIZE_STEP)
      );
      return;
    }

    if (event.key === 'Home') {
      event.preventDefault();
      setWidth(MIN_DETAIL_DRAWER_WIDTH);
      return;
    }

    if (event.key === 'End') {
      event.preventDefault();
      setWidth(MAX_DETAIL_DRAWER_WIDTH);
    }
  };

  return (
    <Drawer
      title={title}
      open={open}
      width={width}
      destroyOnHidden
      onClose={onClose}
      rootClassName="data-model-panel__detail-drawer"
    >
      <div
        aria-label="调整 Data Model 详情宽度"
        aria-orientation="vertical"
        aria-valuemax={MAX_DETAIL_DRAWER_WIDTH}
        aria-valuemin={MIN_DETAIL_DRAWER_WIDTH}
        aria-valuenow={width}
        className="data-model-panel__detail-drawer-resize-handle"
        role="separator"
        tabIndex={0}
        onKeyDown={handleResizeKeyDown}
        onMouseDown={startResize}
      />
      {children}
    </Drawer>
  );
}
