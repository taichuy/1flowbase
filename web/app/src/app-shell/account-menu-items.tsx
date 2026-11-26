import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import LogoutOutlined from '@ant-design/icons/es/icons/LogoutOutlined';
import SwapOutlined from '@ant-design/icons/es/icons/SwapOutlined';
import UserOutlined from '@ant-design/icons/es/icons/UserOutlined';
import type { MenuProps } from 'antd';
import type {
  ConsoleAvailableRole,
  ConsoleWorkspaceSummary
} from '@1flowbase/api-client';
import { i18nText } from '../shared/i18n/text';

export function createAccountMenuItems(
  accountLabel = i18nText('appShell', 'auto.user'),
  roles: ConsoleAvailableRole[] = [],
  activeRoleCode?: string,
  switchingRoleCode?: string | null,
  workspaces: ConsoleWorkspaceSummary[] = [],
  currentWorkspaceId?: string,
  switchingWorkspaceId?: string | null
): MenuProps['items'] {
  const workspaceItem =
    workspaces.length > 1
      ? [
          {
            key: 'active-workspace',
            label: i18nText('appShell', 'auto.current_workspace', {
              defaultValue: 'Workspace'
            }),
            icon: <ApartmentOutlined />,
            children: workspaces.map((workspace) => {
              const isCurrent = currentWorkspaceId
                ? workspace.id === currentWorkspaceId
                : workspace.is_current;
              return {
                key: `workspace:${workspace.id}`,
                label: workspace.name,
                icon: isCurrent ? <CheckOutlined /> : undefined,
                disabled: isCurrent || workspace.id === switchingWorkspaceId
              };
            })
          }
        ]
      : [];

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
        ...workspaceItem,
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
