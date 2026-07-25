import {
  CloseOutlined,
  CompressOutlined,
  FullscreenOutlined
} from '@ant-design/icons';
import { Button, Space, Typography } from 'antd';
import type { ReactNode } from 'react';

export interface BlockStudioWindowHeaderProps {
  closeLabel: string;
  maximized: boolean;
  maximizeLabel: string;
  mobile: boolean;
  restoreLabel: string;
  status: string;
  title: string;
  toolbar: ReactNode;
  onClose: () => void;
  onToggleMaximized: () => void;
}

export function BlockStudioWindowHeader({
  closeLabel,
  maximized,
  maximizeLabel,
  mobile,
  onClose,
  onToggleMaximized,
  restoreLabel,
  status,
  title,
  toolbar
}: BlockStudioWindowHeaderProps) {
  return (
    <header
      className="frontstage-jsx-studio__window-header"
      data-window-drag-handle="true"
    >
      <Space size={8}>
        <Typography.Text strong>{title}</Typography.Text>
        <Typography.Text
          type="secondary"
          className="frontstage-jsx-studio__status"
        >
          {status}
        </Typography.Text>
      </Space>
      <Space className="frontstage-jsx-studio__window-actions" size={8} wrap>
        {toolbar}
        <Button
          aria-label={maximized ? restoreLabel : maximizeLabel}
          disabled={mobile}
          icon={maximized ? <CompressOutlined /> : <FullscreenOutlined />}
          onClick={onToggleMaximized}
        />
        <Button
          aria-label={closeLabel}
          icon={<CloseOutlined />}
          onClick={onClose}
        />
      </Space>
    </header>
  );
}
