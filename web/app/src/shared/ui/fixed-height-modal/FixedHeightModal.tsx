import type { CSSProperties, ReactNode } from 'react';

import { Modal } from 'antd';
import type { ModalProps } from 'antd';

import './fixed-height-modal.css';

export interface FixedHeightModalProps {
  open: boolean;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: ModalProps['width'];
  height?: string;
  className?: string;
  scrollBodyClassName?: string;
  bodyHeader?: ReactNode;
  confirmLoading?: ModalProps['confirmLoading'];
  destroyOnHidden?: ModalProps['destroyOnHidden'];
  onCancel: ModalProps['onCancel'];
  onOk?: ModalProps['onOk'];
}

const DEFAULT_CONTENT_HEIGHT = 'min(700px, calc(100vh - 120px))';

function joinClassNames(...classNames: Array<string | undefined>) {
  return classNames.filter(Boolean).join(' ');
}

export function FixedHeightModal({
  open,
  title,
  children,
  footer,
  width,
  height = DEFAULT_CONTENT_HEIGHT,
  className,
  scrollBodyClassName,
  bodyHeader,
  confirmLoading,
  destroyOnHidden,
  onCancel,
  onOk
}: FixedHeightModalProps) {
  const modalStyle = {
    '--fixed-height-modal-content-height': height
  } as CSSProperties;

  return (
    <Modal
      centered
      className={joinClassNames('fixed-height-modal', className)}
      confirmLoading={confirmLoading}
      destroyOnHidden={destroyOnHidden}
      footer={footer}
      open={open}
      style={modalStyle}
      title={title}
      width={width}
      onCancel={onCancel}
      onOk={onOk}
    >
      <div className="fixed-height-modal__body">
        {bodyHeader ? (
          <div
            className="fixed-height-modal__body-header"
            data-testid="fixed-height-modal-body-header"
          >
            {bodyHeader}
          </div>
        ) : null}
        <div
          className={joinClassNames(
            'fixed-height-modal__scroll-body',
            scrollBodyClassName
          )}
          data-testid="fixed-height-modal-scroll-body"
        >
          {children}
        </div>
      </div>
    </Modal>
  );
}
