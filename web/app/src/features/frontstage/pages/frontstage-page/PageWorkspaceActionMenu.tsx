import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import LayoutOutlined from '@ant-design/icons/es/icons/LayoutOutlined';
import MenuOutlined from '@ant-design/icons/es/icons/MenuOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import TableOutlined from '@ant-design/icons/es/icons/TableOutlined';
import { Dropdown, Select, Switch, Tooltip } from 'antd';
import type { MenuProps } from 'antd';
import type { FC } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { FrontstageNodeActionButton } from '../../components/FrontstageNodeActionButton';
import type { FrontstagePageLayoutMode } from '../../lib/page-document';

type PageWorkspaceActionMenuProps = {
  tabsEnabled: boolean;
  layoutMode: FrontstagePageLayoutMode;
  disabled: boolean;
  refreshing?: boolean;
  onEdit: () => void;
  onRefresh: () => void;
  onTabsEnabledChange: (enabled: boolean) => void;
  onLayoutModeChange: (layoutMode: FrontstagePageLayoutMode) => void;
};

const PageWorkspaceActionMenu: FC<PageWorkspaceActionMenuProps> = ({
  tabsEnabled,
  layoutMode,
  disabled,
  refreshing = false,
  onEdit,
  onRefresh,
  onTabsEnabledChange,
  onLayoutModeChange
}) => {
  const enableTabsLabel = i18nText('frontstage', 'design.enable_tabs');
  const layoutModeLabel = i18nText('frontstage', 'design.layout_mode');
  const actionDisabled = disabled || refreshing;
  const menuItems: MenuProps['items'] = [
    {
      key: 'edit',
      icon: <EditOutlined />,
      label: i18nText('frontstage', 'auto.edit'),
      disabled: actionDisabled,
      onClick: ({ domEvent }) => {
        domEvent.stopPropagation();
        onEdit();
      }
    },
    {
      key: 'refresh',
      icon: <ReloadOutlined spin={refreshing} />,
      label: i18nText('frontstage', 'design.refresh_current_page'),
      disabled: actionDisabled,
      onClick: ({ domEvent }) => {
        domEvent.stopPropagation();
        onRefresh();
      }
    },
    {
      key: 'layout-mode',
      icon: <LayoutOutlined />,
      disabled: actionDisabled,
      label: (
        <div
          className="frontstage-page-workspace__layout-action"
          onClick={(event) => event.stopPropagation()}
        >
          <span>{layoutModeLabel}</span>
          <Select<FrontstagePageLayoutMode>
            aria-label={layoutModeLabel}
            value={layoutMode}
            disabled={actionDisabled}
            size="small"
            style={{ width: 112 }}
            options={[
              {
                value: 'auto',
                label: i18nText('frontstage', 'design.layout_mode_auto')
              },
              {
                value: 'free',
                label: i18nText('frontstage', 'design.layout_mode_free')
              }
            ]}
            onChange={(value) => onLayoutModeChange(value)}
          />
        </div>
      )
    },
    {
      key: 'tabs',
      icon: <TableOutlined />,
      disabled: actionDisabled,
      label: (
        <div className="frontstage-page-workspace__tabs-action">
          <span>{enableTabsLabel}</span>
          <Switch
            aria-label={enableTabsLabel}
            checked={tabsEnabled}
            disabled={actionDisabled}
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
