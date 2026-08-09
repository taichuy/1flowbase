import type { ReactNode } from 'react';

import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';

const DEFAULT_DETAIL_DRAWER_WIDTH = 980;
const MIN_DETAIL_DRAWER_WIDTH = 720;
const MAX_DETAIL_DRAWER_WIDTH = 1280;
const DETAIL_DRAWER_VIEWPORT_GUTTER = 48;

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
  return (
    <ResizableDrawer
      defaultWidth={DEFAULT_DETAIL_DRAWER_WIDTH}
      minWidth={MIN_DETAIL_DRAWER_WIDTH}
      maxWidth={MAX_DETAIL_DRAWER_WIDTH}
      open={open}
      title={title}
      viewportGutter={DETAIL_DRAWER_VIEWPORT_GUTTER}
      destroyOnClose
      rootClassName="data-model-panel__detail-drawer"
      resizeLabel="调整 Data Model 详情宽度"
      onClose={onClose}
    >
      {children}
    </ResizableDrawer>
  );
}
