import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));
const frontstageNavigationApi = vi.hoisted(() => ({
  frontstagePageTreeQueryKey: vi.fn((workspaceId: string) => [
    'frontstage',
    workspaceId,
    'page-tree'
  ]),
  fetchFrontstagePageTree: vi.fn()
}));

vi.mock(
  '../../features/settings/api/console-navigation',
  () => consoleNavigationApi
);
vi.mock(
  '../../features/frontstage/api/page-tree',
  () => frontstageNavigationApi
);

import { AppProviders } from '../../app/AppProviders';
import { Navigation } from '../Navigation';
import { appI18n } from '../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../state/frontstage-design-mode-store';

const primaryRouteRecords = {
  home: {
    path: '/',
    label_key: 'auto.workbench'
  },
  'embedded-apps': {
    path: '/embedded-apps',
    label_key: 'auto.subsystem'
  },
  templates: {
    path: '/templates',
    label_key: 'auto.templates'
  }
} as const;

function consoleNavigationForPrimaryRoutes(
  routeIds: Array<keyof typeof primaryRouteRecords>
) {
  return {
    route_definitions: routeIds.map((route_id) => ({
      route_id,
      surface_key: route_id,
      path: primaryRouteRecords[route_id].path,
      surface_kind: 'system' as const
    })),
    navigation_items: routeIds.map((route_id, index) => ({
      item_id: route_id,
      route_id,
      parent_item_id: null,
      label_key: primaryRouteRecords[route_id].label_key,
      navigation_slot: 'primary' as const,
      order: index + 1
    })),
    permission_bindings: []
  };
}

function renderNavigation(pathname: string) {
  return render(
    <AppProviders>
      <Navigation pathname={pathname} useRouterLinks={false} />
    </AppProviders>
  );
}

describe('Navigation', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    resetFrontstageDesignModeStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForPrimaryRoutes([
        'home',
        'embedded-apps',
        'templates'
      ])
    );
    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([]);
  });

  test('AC-001 renders topbar pages from the same accessible frontstage navigation tree', async () => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'normal-user',
        effective_display_role: 'developer',
        current_workspace_id: 'workspace-123'
      },
      me: null
    });
    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([
      {
        id: 'group-sales',
        title: '销售',
        kind: 'group',
        placement: 'topbar',
        slug: 'sales',
        children: [
          {
            id: 'page-sales',
            title: '销售看板',
            kind: 'page',
            placement: 'sidebar',
            children: []
          }
        ]
      },
      {
        id: 'page-internal',
        title: '内部页面',
        kind: 'page',
        placement: 'sidebar',
        children: []
      }
    ]);

    renderNavigation('/sales/pages/page-sales/tabs/tab-1');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    expect(
      await within(nav).findByRole('link', { name: '销售' })
    ).toHaveAttribute('href', '/sales');
    expect(within(nav).getByRole('link', { name: '销售' })).toHaveAttribute(
      'aria-current',
      'page'
    );
    expect(
      within(nav).queryByRole('link', { name: '销售看板' })
    ).not.toBeInTheDocument();
    expect(
      within(nav).queryByRole('link', { name: '内部页面' })
    ).not.toBeInTheDocument();
    expect(
      within(nav).getByRole('link', { name: '工作台' })
    ).toBeInTheDocument();
    expect(within(nav).getByRole('link', { name: '模板' })).toBeInTheDocument();
  });

  test('AC-001 and AC-002 reuse the sidebar add action and let navigation fill remaining width', async () => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'developer',
        effective_display_role: 'developer',
        current_workspace_id: 'workspace-123'
      },
      me: null
    });
    useFrontstageDesignModeStore.getState().setDesignMode(true);

    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([
      {
        id: 'group-new',
        title: '新增菜单',
        kind: 'group',
        placement: 'topbar',
        slug: 'new-space',
        children: []
      }
    ]);

    renderNavigation('/templates');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    expect(nav).toHaveClass('app-shell-navigation');
    expect(within(nav).getByRole('menu')).toHaveClass('app-shell-menu');
    expect(within(nav).getByRole('button', { name: '添加菜单' })).toHaveClass(
      'frontstage-add-action-button',
      'frontstage-add-action-button--compact'
    );
    expect(
      within(nav).getByRole('button', { name: '添加菜单' })
    ).toHaveTextContent('添加菜单');
    expect(
      await within(nav).findByRole('link', { name: '新增菜单' })
    ).toHaveAttribute('href', '/new-space');
    expect(
      within(nav).queryByRole('button', { name: '管理顶部导航' })
    ).not.toBeInTheDocument();
    const topLevelItems = within(nav).getAllByRole('menuitem');
    expect(topLevelItems.map((item) => item.textContent)).toEqual([
      '工作台',
      '子系统',
      '模板',
      '新增菜单'
    ]);
  });

  test('AC-006 creates topbar nodes with title and refreshable slug fields', async () => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'developer',
        effective_display_role: 'developer',
        current_workspace_id: 'workspace-123'
      },
      me: null
    });
    useFrontstageDesignModeStore.getState().setDesignMode(true);
    renderNavigation('/templates');

    fireEvent.click(await screen.findByRole('button', { name: '添加菜单' }));
    fireEvent.click(await screen.findByText('新增菜单'));

    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByRole('textbox', { name: '名称' })
    ).toBeInTheDocument();
    const slugInput = within(dialog).getByRole('textbox', { name: '访问路径' });
    const initialSlug = (slugInput as HTMLInputElement).value;
    expect(initialSlug).toMatch(/^p[a-z0-9]{7}$/);
    fireEvent.click(
      within(dialog).getByRole('button', { name: '刷新访问路径' })
    );
    expect(slugInput).not.toHaveValue(initialSlug);
  });

  test('renders primary console navigation and keeps settings out of the primary rail', async () => {
    resetAuthStore();

    renderNavigation('/embedded-apps');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });

    expect(
      await within(nav).findByRole('link', { name: '工作台' })
    ).toBeInTheDocument();
    expect(
      within(nav).getByRole('link', { name: '子系统' })
    ).toBeInTheDocument();
    expect(within(nav).getByRole('link', { name: '模板' })).toHaveAttribute(
      'href',
      '/templates'
    );
    expect(
      within(nav).queryByRole('link', { name: '设置' })
    ).not.toBeInTheDocument();
    expect(
      await screen.findByRole('link', { name: '子系统', current: 'page' })
    ).toBeInTheDocument();
  });

  test('uses backend primary navigation without expanding from permissions', async () => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'normal-user',
        effective_display_role: 'developer',
        current_workspace_id: 'workspace-123'
      },
      me: {
        id: 'user-1',
        account: 'normal-user',
        email: 'normal-user@example.com',
        phone: null,
        nickname: 'Normal User',
        name: 'Normal User',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'developer',
        permissions: ['embedded_app.view.all', 'template.view.all']
      }
    });
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForPrimaryRoutes(['embedded-apps'])
    );

    renderNavigation('/embedded-apps');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    await waitFor(() => {
      expect(
        within(nav).queryByRole('link', { name: '工作台' })
      ).not.toBeInTheDocument();
    });
    expect(within(nav).getByRole('link', { name: '子系统' })).toHaveAttribute(
      'href',
      '/embedded-apps'
    );
    expect(
      within(nav).queryByRole('link', { name: '模板' })
    ).not.toBeInTheDocument();
  });

  test('shows registry error instead of falling back to local primary routes', async () => {
    resetAuthStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockRejectedValue(
      new Error('registry unavailable')
    );

    renderNavigation('/embedded-apps');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    expect(
      await within(nav).findByText('控制台导航加载失败')
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(
        within(nav).queryByRole('link', { name: '工作台' })
      ).not.toBeInTheDocument();
    });
    expect(
      within(nav).queryByRole('link', { name: '子系统' })
    ).not.toBeInTheDocument();
    expect(
      within(nav).queryByRole('link', { name: '模板' })
    ).not.toBeInTheDocument();
  });
});
