import type { PropsWithChildren } from 'react';

import { AppShell } from '@1flowbase/ui';
import { Link } from '@tanstack/react-router';
import { Space } from 'antd';
import { useTranslation } from 'react-i18next';

import { AccountMenu } from './AccountMenu';
import { FrontstageDesignModeAction } from './FrontstageDesignModeAction';
import { HelpChromeMenu } from './HelpChromeMenu';
import { LanguageChromeMenu } from './LanguageChromeMenu';
import { Navigation } from './Navigation';
import { SettingsChromeMenu } from './SettingsChromeMenu';
import { getSecondaryChromeRoutes } from '../routes/route-helpers';
import './app-shell.css';

function renderActionLink(
  pathname: string,
  label: string,
  useRouterLinks: boolean,
  isCurrent: boolean
) {
  if (useRouterLinks) {
    return (
      <Link
        to={pathname}
        className="app-shell-menu-link"
        aria-current={isCurrent ? 'page' : undefined}
      >
        {label}
      </Link>
    );
  }

  return (
    <a
      href={pathname}
      className="app-shell-menu-link"
      aria-current={isCurrent ? 'page' : undefined}
    >
      {label}
    </a>
  );
}

export function AppShellFrame({
  children,
  pathname = '/',
  useRouterLinks = false
}: PropsWithChildren<{ pathname?: string; useRouterLinks?: boolean }>) {
  const { t } = useTranslation('appShell');
  const secondaryActions = getSecondaryChromeRoutes();

  return (
    <AppShell
      title="1flowbase"
      navigation={<Navigation pathname={pathname} useRouterLinks={useRouterLinks} />}
      actions={
        <Space className="app-shell-action-row" size={20}>
          <span className="app-shell-secondary-actions">
            <FrontstageDesignModeAction />
            {secondaryActions.map((route) => (
              <span key={route.id}>
                {route.id === 'settings' ? (
                  <SettingsChromeMenu
                    pathname={pathname}
                    useRouterLinks={useRouterLinks}
                  />
                ) : (
                  renderActionLink(
                    route.path,
                    t(route.navLabelKey!),
                    useRouterLinks,
                    route.selectedMatchers.some((match) => match(pathname))
                  )
                )}
              </span>
            ))}
            <HelpChromeMenu />
            <LanguageChromeMenu />
          </span>
          <AccountMenu useRouterNavigation={useRouterLinks} />
        </Space>
      }
    >
      {children}
    </AppShell>
  );
}
