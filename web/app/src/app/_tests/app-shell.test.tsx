import fs from 'node:fs';
import path from 'node:path';

import { render, screen, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  getDefaultApiBaseUrl: vi.fn().mockReturnValue('http://127.0.0.1:7800'),
  getConsoleNavigation: vi.fn().mockResolvedValue({
    route_definitions: [
      {
        route_id: 'home',
        surface_key: 'home',
        path: '/',
        surface_kind: 'system'
      },
      {
        route_id: 'embedded-apps',
        surface_key: 'embedded-apps',
        path: '/embedded-apps',
        surface_kind: 'system'
      },
      {
        route_id: 'templates',
        surface_key: 'templates',
        path: '/templates',
        surface_kind: 'system'
      },
      {
        route_id: 'settings.api-key-authentication',
        surface_key: 'api-key-authentication',
        path: '/settings/api-key-authentication',
        surface_kind: 'system'
      }
    ],
    navigation_items: [
      {
        item_id: 'home',
        route_id: 'home',
        parent_item_id: null,
        label_key: 'auto.workbench',
        navigation_slot: 'primary',
        order: 1
      },
      {
        item_id: 'embedded-apps',
        route_id: 'embedded-apps',
        parent_item_id: null,
        label_key: 'auto.subsystem',
        navigation_slot: 'primary',
        order: 3
      },
      {
        item_id: 'templates',
        route_id: 'templates',
        parent_item_id: null,
        label_key: 'auto.templates',
        navigation_slot: 'primary',
        order: 4
      },
      {
        item_id: 'settings.api-key-authentication',
        route_id: 'settings.api-key-authentication',
        parent_item_id: 'settings',
        label_key: 'auto.api_key_authentication',
        navigation_slot: 'settings',
        order: 1
      }
    ],
    permission_bindings: []
  }),
  getConsoleApplicationCatalog: vi.fn().mockResolvedValue({
    types: [{ value: 'agent_flow', label: 'AgentFlow' }],
    tags: []
  }),
  listConsoleApplications: vi.fn().mockResolvedValue([
    {
      id: 'app-1',
      application_type: 'agent_flow',
      name: 'Support Agent',
      description: 'customer support',
      icon: 'RobotOutlined',
      icon_type: 'iconfont',
      icon_background: '#E6F7F2',
      created_by: 'user-1',
      updated_at: '2026-04-15T09:00:00Z',
      tags: []
    }
  ]),
  fetchConsoleRuntimeModelRecords: vi.fn().mockResolvedValue({ items: [], total: 0 }),
  createConsoleRuntimeModelRecord: vi.fn().mockResolvedValue({}),
  updateConsoleRuntimeModelRecord: vi.fn().mockResolvedValue({}),
  deleteConsoleRuntimeModelRecord: vi.fn().mockResolvedValue({ deleted: true }),
  dispatchFrontstageQuery: vi.fn(),
  dispatchFrontstageAction: vi.fn()
}));

vi.mock('../../features/auth/components/AuthBootstrap', () => ({
  AuthBootstrap: ({ children }: { children: ReactNode }) => children
}));

import { useAuthStore } from '../../state/auth-store';
import { App } from '../App';

describe('App shell', () => {
  beforeEach(() => {
    window.history.pushState({}, '', '/');
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Captain Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'member',
        permissions: ['embedded_app.view.all']
      }
    });
  });

  test(
    'renders the formal console shell with application workspace content',
    async () => {
      render(<App />);

      expect(await screen.findByRole('heading', { name: '1flowbase' })).toBeInTheDocument();

      const header = screen.getByRole('banner');
      const primaryNavigation = screen.getByRole('navigation', { name: 'Primary' });

      expect(header).not.toHaveStyle('--app-shell-edge-gap: 5%');
      expect(within(primaryNavigation).getByRole('menu')).toBeInTheDocument();
      expect(
        await within(primaryNavigation).findByRole('link', { name: '工作台' })
      ).toBeInTheDocument();
      expect(
        within(primaryNavigation).getByRole('link', { name: '子系统' })
      ).toBeInTheDocument();
      expect(
        within(primaryNavigation).getByRole('link', { name: '模板' })
      ).toBeInTheDocument();
      expect(screen.getByRole('menuitem', { name: '设置' })).toBeInTheDocument();
      expect(screen.getByRole('menuitem', { name: 'Captain Root' })).toBeInTheDocument();
      expect(
        within(primaryNavigation).queryByRole('link', { name: 'Home' })
      ).not.toBeInTheDocument();
      expect(
        within(primaryNavigation).queryByRole('link', { name: 'Embedded Apps' })
      ).not.toBeInTheDocument();
      expect(
        within(primaryNavigation).queryByRole('link', { name: 'Agent Flow' })
      ).not.toBeInTheDocument();
      expect(screen.queryByText('Workspace Bootstrap')).not.toBeInTheDocument();
      expect(screen.queryByRole('link', { name: 'Theme Preview' })).not.toBeInTheDocument();
      expect(await screen.findByText('Support Agent')).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: '进入应用' })).not.toBeInTheDocument();
      expect(screen.getByRole('link', { name: '进入应用-Support Agent' })).toHaveAttribute(
        'href',
        '/applications/app-1/orchestration'
      );
      expect(screen.getByRole('button', { name: '更多操作-Support Agent' })).toBeInTheDocument();
      expect(screen.queryByText(/api-server/i)).not.toBeInTheDocument();
    },
    15000
  );

  test('renders the embedded apps route', async () => {
    window.history.pushState({}, '', '/embedded-apps');

    render(<App />);

    expect(
      await screen.findByRole('heading', { name: '子系统', level: 2 })
    ).toBeInTheDocument();
  });

  test('keeps the shell content container full width instead of capping to 1200px', () => {
    const appShellCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../../app-shell/app-shell.css'),
      'utf8'
    );

    expect(appShellCss).not.toContain('width: min(1200px, calc(100% - 48px));');
    expect(appShellCss).toContain('width: 100%;');
    expect(appShellCss).toContain('padding: 0;');
    expect(appShellCss).toContain('box-sizing: border-box;');
    expect(appShellCss).not.toContain('padding: 28px 24px 64px;');
    expect(appShellCss).not.toContain('margin: 0 auto;');
  });

  test.each(['/agent-flow', '/embedded/demo-app', '/embedded-apps/demo-app'])(
    'no longer resolves legacy console route %s',
    async (pathname) => {
      window.history.pushState({}, '', pathname);

      render(<App />);

      expect(await screen.findByText('页面不存在')).toBeInTheDocument();
    }
  );
});
