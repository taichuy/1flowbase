import { Modal } from 'antd';
import type { ReactNode } from 'react';

import type { ModalPanelSchema } from '../contracts/overlay-shell-schema';

export function SchemaModalPanel({
  open,
  schema,
  children,
  footer = null,
  onClose
}: {
  open: boolean;
  schema: ModalPanelSchema;
  children: ReactNode;
  footer?: ReactNode | null;
  onClose: () => void;
}) {
  return (
    <Modal
      destroyOnHidden={schema.destroyOnHidden}
      footer={footer}
      open={open}
      title={schema.title}
      width={schema.width}
      onCancel={onClose}
    >
      {children}
    </Modal>
  );
}
