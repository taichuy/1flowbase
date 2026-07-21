import { App as AntdApp, ConfigProvider, Layout, Typography } from 'antd';
import type { PropsWithChildren, ReactNode } from 'react';

import { emeraldLightTheme } from './theme';

const { Header, Content } = Layout;

export function AppThemeProvider({ children }: PropsWithChildren) {
  return (
    <ConfigProvider theme={emeraldLightTheme}>
      <AntdApp>{children}</AntdApp>
    </ConfigProvider>
  );
}

export interface AppShellProps extends PropsWithChildren {
  title: string;
  navigation?: ReactNode;
  actions?: ReactNode;
}

export function AppShell({ title, navigation, actions, children }: AppShellProps) {
  return (
    <Layout className="app-shell">
      <Header
        className="app-shell-header"
        data-window-workspace-top-inset
        role="banner"
      >
        <div className="app-shell-header-main">
          <div className="app-shell-brand">
            <img className="app-shell-logo" src="/icon.svg" alt="" aria-hidden="true" />
            <Typography.Title level={4} className="app-shell-title">
              {title}
            </Typography.Title>
          </div>
          <div className="app-shell-nav">{navigation}</div>
        </div>
        {actions ? <div className="app-shell-actions">{actions}</div> : null}
      </Header>
      <Content className="app-shell-content">{children}</Content>
    </Layout>
  );
}
