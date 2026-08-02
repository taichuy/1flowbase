import { type ReactNode } from 'react';
import { Menu } from 'antd';

import { AppRouterProvider } from '../app/router';
import { AppShellFrame } from '../app-shell/AppShellFrame';
import { createAccountMenuItems } from '../app-shell/account-menu-items';
import { AgentFlowEditorShell } from '../features/agent-flow/components/editor/AgentFlowEditorShell';
import { EmbeddedAppsPage } from '../features/embedded-apps/pages/EmbeddedAppsPage';
import { FrontStagePage } from '../features/frontstage/pages/FrontStagePage';
import { SchemaFormDrawer } from '../shared/schema-ui/v1/form-drawer/SchemaFormDrawer';
import { TemplatesPage } from '../features/templates/pages/TemplatesPage';
import {
  createStyleBoundaryFrontstagePageContent,
  createStyleBoundaryOrchestrationState,
  seedStyleBoundaryApplicationFetch,
  seedStyleBoundaryAuth,
  seedStyleBoundaryCommonFetch,
  seedStyleBoundaryFrontstageFetch,
  seedStyleBoundarySettingsFetch,
  seedStyleBoundaryTemplateFetch
} from './scene-fixtures';
import { SettingsMcpManagementStyleBoundaryScene } from './SettingsMcpManagementStyleBoundaryScene';
import { SettingsSystemRuntimeStyleBoundaryScene } from './SettingsSystemRuntimeStyleBoundaryScene';
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

function renderRouterScene(
  pathname: string,
  options: { authenticated?: boolean } = {}
) {
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
          nodeCatalog={{ nodes: [] }}
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
  'component.schema-form-drawer': () => (
    <SchemaFormDrawer
      open
      title="Password 配置"
      schema={{
        schema_version: '1.0.0',
        fields: [
          {
            key: 'name',
            label: '标识',
            type: 'string',
            read_only: true
          },
          {
            key: 'title',
            label: '名称',
            type: 'string',
            required: true
          },
          {
            key: 'description',
            label: '说明',
            type: 'string',
            control: 'textarea'
          },
          {
            key: 'enabled',
            label: '启用',
            type: 'boolean'
          }
        ]
      }}
      initialValues={{
        name: 'password-local',
        title: 'Password',
        description: 'Local password authentication',
        enabled: true
      }}
      onCancel={() => undefined}
      onSubmit={() => undefined}
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
  'page.templates': () => {
    seedStyleBoundaryTemplateFetch();
    return renderShellScene('/templates', <TemplatesPage />);
  },
  'page.settings-extension-center-agent-flow': () => {
    seedStyleBoundaryTemplateFetch();
    return renderRouterScene('/settings/extension-center/agent-flow');
  },
  'page.settings': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/model-providers');
  },
  'page.settings-i18n.desktop': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/i18n');
  },
  'page.settings-i18n.mobile': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/i18n');
  },
  'page.settings-applications': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/applications');
  },
  'page.settings-system-runtime': () => {
    seedStyleBoundarySettingsFetch();
    return <SettingsSystemRuntimeStyleBoundaryScene />;
  },
  'page.settings-mcp-management': () => {
    seedStyleBoundarySettingsFetch();
    return <SettingsMcpManagementStyleBoundaryScene />;
  },
  'page.settings-docs': () => {
    seedStyleBoundarySettingsFetch();
    return renderRouterScene('/settings/docs?category=console');
  },
  'page.me': () => renderRouterScene('/me/profile'),
  'page.sign-in': () => renderRouterScene('/sign-in', { authenticated: false })
};
