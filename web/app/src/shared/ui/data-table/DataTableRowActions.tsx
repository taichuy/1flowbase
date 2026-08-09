import { MoreOutlined } from '@ant-design/icons';
import { Button, Dropdown, Space } from 'antd';
import type { MenuProps } from 'antd';
import type { ReactNode } from 'react';

export function DataTableRowActions({
  children,
  moreAriaLabel,
  moreItems,
  onMoreAction
}: {
  children: ReactNode;
  moreAriaLabel: string;
  moreItems: MenuProps['items'];
  onMoreAction: (key: string) => void;
}) {
  return (
    <Space size={2}>
      {children}
      <Dropdown
        menu={{
          items: moreItems,
          onClick: ({ key }) => onMoreAction(key)
        }}
        trigger={['click']}
      >
        <Button
          aria-label={moreAriaLabel}
          icon={<MoreOutlined />}
          size="small"
          type="text"
        />
      </Dropdown>
    </Space>
  );
}
