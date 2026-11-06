import MoreOutlined from '@ant-design/icons/es/icons/MoreOutlined';
import { Button, Dropdown, Space, Tooltip } from 'antd';
import type { ReactNode } from 'react';

export type DataTableRowAction = {
  key: string;
  label: ReactNode;
  icon?: ReactNode;
  disabled?: boolean;
  danger?: boolean;
  href?: string;
  tooltip?: ReactNode;
  onClick?: () => void;
};

export function DataTableRowActions({
  actions,
  moreAriaLabel
}: {
  actions: DataTableRowAction[];
  moreAriaLabel: string;
}) {
  const inlineActions = actions.slice(0, 2);
  const overflowActions = actions.slice(2);
  if (!actions.length) return null;

  return (
    <Space size={2}>
      {inlineActions.map((action) => {
        const button = (
          <Button
            disabled={action.disabled}
            danger={action.danger}
            href={action.href}
            icon={action.icon}
            onClick={action.onClick}
            size="small"
            type="link"
          >
            {action.label}
          </Button>
        );
        return (
          <Tooltip key={action.key} title={action.tooltip}>
            <span>{button}</span>
          </Tooltip>
        );
      })}
      {overflowActions.length > 0 && (
        <Dropdown
          menu={{
            items: overflowActions.map((action) => ({
              key: action.key,
              label:
                action.href && !action.disabled ? (
                  <a href={action.href}>{action.label}</a>
                ) : (
                  action.label
                ),
              icon: action.icon,
              disabled: action.disabled,
              danger: action.danger,
              onClick: action.onClick
            }))
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
      )}
    </Space>
  );
}
