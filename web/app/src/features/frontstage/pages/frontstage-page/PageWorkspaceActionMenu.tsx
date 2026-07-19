import {
  EditOutlined,
  MenuOutlined,
  TableOutlined
} from '@ant-design/icons';
import { Dropdown, Switch, Tooltip } from 'antd';
import type { MenuProps } from 'antd';
import type { FC } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { FrontstageNodeActionButton } from '../../components/FrontstageNodeActionButton';

type PageWorkspaceActionMenuProps = {
  tabsEnabled: boolean;
  disabled: boolean;
  onEdit: () => void;
  onTabsEnabledChange: (enabled: boolean) => void;
};

const PageWorkspaceActionMenu: FC<PageWorkspaceActionMenuProps> = ({
  tabsEnabled,
  disabled,
  onEdit,
  onTabsEnabledChange
}) => {
  const enableTabsLabel = i18nText('frontstage', 'design.enable_tabs');
  const menuItems: MenuProps['items'] = [
    {
      key: 'edit',
      icon: <EditOutlined />,
      label: i18nText('frontstage', 'auto.edit'),
      disabled,
      onClick: ({ domEvent }) => {
        domEvent.stopPropagation();
        onEdit();
      }
    },
    {
      key: 'tabs',
      icon: <TableOutlined />,
      disabled,
      label: (
        <div className="frontstage-page-workspace__tabs-action">
          <span>{enableTabsLabel}</span>
          <Switch
            aria-label={enableTabsLabel}
            checked={tabsEnabled}
            disabled={disabled}
            size="small"
            onChange={(checked, event) => {
              event.stopPropagation();
              onTabsEnabledChange(checked);
            }}
          />
        </div>
      )
    }
  ];

  return (
    <Dropdown
      menu={{ items: menuItems }}
      placement="bottomRight"
      trigger={['click']}
    >
      <Tooltip title={i18nText('frontstage', 'design.configure_page')}>
        <FrontstageNodeActionButton
          aria-label={i18nText('frontstage', 'design.configure_page')}
          disabled={disabled}
          icon={<MenuOutlined />}
          onClick={(event) => event.stopPropagation()}
        />
      </Tooltip>
    </Dropdown>
  );
};

export { PageWorkspaceActionMenu };
