import { useState, type CSSProperties, type ReactNode } from 'react';
import { ModalWidthResize } from './ModalWidthResize';

import { Modal } from 'antd';
import type { ModalProps } from 'antd';

import './fixed-height-modal.css';

export interface FixedHeightModalProps {
  open: boolean;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: ModalProps['width'];
  resizable?: boolean;
  height?: string;
  className?: string;
  scrollBodyClassName?: string;
  bodyHeader?: ReactNode;
  confirmLoading?: ModalProps['confirmLoading'];
  okText?: ModalProps['okText'];
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
  resizable = false,
  height = DEFAULT_CONTENT_HEIGHT,
  className,
  scrollBodyClassName,
  bodyHeader,
  confirmLoading,
  okText,
  destroyOnHidden,
  onCancel,
  onOk
}: FixedHeightModalProps) {
  const [resizedWidth, setResizedWidth] = useState<number>();
  const modalStyle = {
    ...(resizable
      ? {
          minWidth: 'min(560px, calc(100vw - 32px))',
          maxWidth: 'calc(100vw - 32px)'
        }
      : {}),
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
      okText={okText}
      style={modalStyle}
      title={title}
      width={resizable ? (resizedWidth ?? width) : width}
      afterClose={() => setResizedWidth(undefined)}
      modalRender={
        resizable
          ? (node) => (
              <ModalWidthResize onWidthChange={setResizedWidth}>
                {node}
              </ModalWidthResize>
            )
          : undefined
      }
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
