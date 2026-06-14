import { type ReactNode } from 'react';
import { Menu } from 'antd';

import { AppRouterProvider } from '../app/router';
import { AppShellFrame } from '../app-shell/AppShellFrame';
import { createAccountMenuItems } from '../app-shell/account-menu-items';
import { AgentFlowEditorShell } from '../features/agent-flow/components/editor/AgentFlowEditorShell';
import { EmbeddedAppsPage } from '../features/embedded-apps/pages/EmbeddedAppsPage';
import { FrontStagePage } from '../features/frontstage/pages/FrontStagePage';
import { ToolsPage } from '../features/tools/pages/ToolsPage';
import {
  createStyleBoundaryFrontstagePageContent,
  createStyleBoundaryOrchestrationState,
  seedStyleBoundaryApplicationFetch,
  seedStyleBoundaryAuth,
  seedStyleBoundaryCommonFetch,
  seedStyleBoundaryFrontstageFetch,
  seedStyleBoundarySettingsFetch,
  styleBoundaryNodeContributions
} from './scene-fixtures';
import { useAuthStore } from '../state/auth-store';
import type { StyleBoundaryRuntimeScene } from './types';

function getAccountPopupChildren() {
  const items = createAccountMenuItems() ?? [];
  const firstItem = items[0];

  if (
    !firstItem ||
    typeof firstItem !== 'object' ||
    !('children' in firstItem) ||
    !Array.isArray(firstItem.children)
  ) {
    return [];
  }

  return firstItem.children;
}

function renderShellScene(pathname: string, page: ReactNode) {
  seedStyleBoundaryCommonFetch();
  seedStyleBoundaryAuth();

  return <AppShellFrame pathname={pathname}>{page}</AppShellFrame>;
}

function renderRouterScene(pathname: string, options: { authenticated?: boolean } = {}) {
  seedStyleBoundaryCommonFetch();
  if (options.authenticated === false) {
    useAuthStore.getState().setAnonymous();
  } else {
    seedStyleBoundaryAuth();
  }
  window.history.replaceState({}, '', pathname);

  return <AppRouterProvider />;
}

export const renderers: Record<string, StyleBoundaryRuntimeScene['render']> = {
  'component.agent-flow-node-detail': () => {
    seedStyleBoundaryAuth();
    seedStyleBoundaryApplicationFetch();

    return (
      <div style={{ width: 1280, height: 800 }}>
        <AgentFlowEditorShell
          applicationId="app-1"
          applicationName="Support Agent"
          initialState={createStyleBoundaryOrchestrationState()}
          nodeContributions={styleBoundaryNodeContributions}
        />
      </div>
    );
  },
  'component.account-popup': () => (
    <div className="app-shell-account-popup">
      <Menu
        mode="vertical"
        selectable={false}
        items={getAccountPopupChildren()}
      />
    </div>
  ),
  'component.account-trigger': () => (
    <Menu
      className="app-shell-account-menu"
      mode="horizontal"
      selectable={false}
      items={createAccountMenuItems()}
      openKeys={['account']}
    />
  ),
  'page.home': () => {
    seedStyleBoundaryApplicationFetch();
    return renderRouterScene('/');
  },
  'page.frontstage': () => {
    seedStyleBoundaryFrontstageFetch();

    return renderShellScene(
      '/frontstage',
      <FrontStagePage
        workspaceId="workspace-1"
        pageId="page-1"
        initialPageTree={[{ id: 'page-1', title: 'Landing', kind: 'page' }]}
        pageContent={createStyleBoundaryFrontstagePageContent()}
      />
    );
  },
  'page.application-detail': () => {
    seedStyleBoundaryApplicationFetch();
    return renderRouterScene('/applications/app-1/orchestration');
  },
  'page.application-api': () => {
    seedStyleBoundaryApplicationFetch();
    return renderRouterScene('/applications/app-1/api');
  },
  'page.application-logs': () => {
    seedStyleBoundaryApplicationFetch();
    return renderRouterScene('/applications/app-1/logs');
  },
  'page.embedded-apps': () =>
    renderShellScene('/embedded-apps', <EmbeddedAppsPage />),
  'page.tools': () => renderShellScene('/tools', <ToolsPage />),
  'page.settings': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/model-providers');
  },
  'page.settings-docs': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/docs?category=console');
  },
  'page.me': () => renderRouterScene('/me/profile'),
  'page.sign-in': () => renderRouterScene('/sign-in', { authenticated: false })
};
