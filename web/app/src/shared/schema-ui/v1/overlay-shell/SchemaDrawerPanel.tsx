import { Drawer } from 'antd';
import type { ReactNode } from 'react';

import type { DrawerPanelSchema } from '../contracts/overlay-shell-schema';

export function SchemaDrawerPanel({
  open,
  schema,
  children,
  onClose
}: {
  open: boolean;
  schema: DrawerPanelSchema;
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <Drawer
      destroyOnHidden={schema.destroyOnClose}
      getContainer={schema.getContainer}
      open={open}
      placement="right"
      title={schema.title}
      size={schema.width}
      onClose={onClose}
    >
      {children}
    </Drawer>
  );
}
