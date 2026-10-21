import { useSystemMetadata } from '../shared/api/system-metadata';
import { NotificationBell } from '../features/notifications/components/NotificationBell';
import { useLayoutEffect, type PropsWithChildren } from 'react';

import { AppShell } from '@1flowbase/ui';
import { Link } from '@tanstack/react-router';
import { Space } from 'antd';
import { useTranslation } from 'react-i18next';

import { AccountMenu } from './AccountMenu';
import { AssistantClientToolProvider } from './AssistantClientTools';
import { useAssistantClientTools } from './assistant-client-tools/context';
import { FrontstageDesignModeAction } from './FrontstageDesignModeAction';
import { HelpChromeMenu } from './HelpChromeMenu';
import { LanguageChromeMenu } from './LanguageChromeMenu';
import { Navigation } from './Navigation';
import { SettingsChromeMenu } from './SettingsChromeMenu';
import { getSecondaryChromeRoutes } from '../routes/route-helpers';
import { EmbeddedAgentAssistant } from '../features/agent-flow/components/embedded-assistant/EmbeddedAgentAssistant';
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

function AppShellFrameContent({
  children,
  pathname = '/',
  useRouterLinks = false
}: PropsWithChildren<{ pathname?: string; useRouterLinks?: boolean }>) {
  const { t } = useTranslation('appShell');
  const { data: metadata } = useSystemMetadata();
  const secondaryActions = getSecondaryChromeRoutes();
  const assistantClientTools = useAssistantClientTools();

  useLayoutEffect(() => {
    if (performance.getEntriesByName('1flowbase:ShellReady').length === 0) {
      performance.mark('1flowbase:ShellReady');
    }
  }, []);

  return (
    <AppShell
      title={metadata?.system_name ?? ''}
      logoUrl={metadata?.logo_url}
      navigation={
        <Navigation pathname={pathname} useRouterLinks={useRouterLinks} />
      }
      actions={
        <Space className="app-shell-action-row" size={20}>
          <span className="app-shell-secondary-actions">
            <span className="app-shell-mode-actions">
              <FrontstageDesignModeAction />
              <EmbeddedAgentAssistant
                clientTools={assistantClientTools}
                pageKey={pathname}
              />
            </span>
            <NotificationBell />
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

export function AppShellFrame(
  props: PropsWithChildren<{
    pathname?: string;
    useRouterLinks?: boolean;
  }>
) {
  return (
    <AssistantClientToolProvider>
      <AppShellFrameContent {...props} />
    </AssistantClientToolProvider>
  );
}
