import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import LogoutOutlined from '@ant-design/icons/es/icons/LogoutOutlined';
import SwapOutlined from '@ant-design/icons/es/icons/SwapOutlined';
import UserOutlined from '@ant-design/icons/es/icons/UserOutlined';
import type { MenuProps } from 'antd';
import type { ConsoleAvailableRole } from '@1flowbase/api-client';
import { i18nText } from '../shared/i18n/text';

export function createAccountMenuItems(
  accountLabel = i18nText('appShell', 'auto.user'),
  roles: ConsoleAvailableRole[] = [],
  activeRoleCode?: string,
  switchingRoleCode?: string | null
): MenuProps['items'] {
  return [
    {
      key: 'account',
      label: (
        <span aria-label={accountLabel} className="app-shell-account-block">
          <UserOutlined className="app-shell-account-icon" />
          <span className="app-shell-account-label">{accountLabel}</span>
        </span>
      ),
      popupClassName: 'app-shell-account-popup',
      children: [
        {
          key: 'profile',
          label: i18nText('appShell', 'auto.profile'),
          icon: <UserOutlined />
        },
        {
          key: 'active-role',
          label: i18nText('appShell', 'auto.current_role', {
            defaultValue: 'Current role'
          }),
          icon: <SwapOutlined />,
          children: roles.map((role) => ({
            key: `role:${role.code}`,
            label: role.name,
            icon: role.code === activeRoleCode ? <CheckOutlined /> : undefined,
            disabled:
              role.code === activeRoleCode || role.code === switchingRoleCode
          }))
        },
        {
          key: 'sign-out',
          label: i18nText('appShell', 'auto.logout'),
          icon: <LogoutOutlined />
        }
      ]
    }
  ];
}
