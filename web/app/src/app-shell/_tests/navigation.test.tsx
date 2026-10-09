import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const runtimeI18nApi = vi.hoisted(() => ({
  fetchFrontstageRuntimeI18nCatalog: vi.fn()
}));
vi.mock('../../features/frontstage/api/runtime-i18n', () => runtimeI18nApi);

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
  'embedded-apps': {
    path: '/embedded-apps',
    label_key: 'auto.subsystem'
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
    runtimeI18nApi.fetchFrontstageRuntimeI18nCatalog.mockImplementation(
      async (locale: string) => ({ locale, messages: {}, catalog_revision: 1, digest: locale })
    );
    resetFrontstageDesignModeStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForPrimaryRoutes(['embedded-apps'])
    );
    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([]);
  });

  test('translates dynamic desktop and nested mobile labels on language changes', async () => {
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
    runtimeI18nApi.fetchFrontstageRuntimeI18nCatalog.mockImplementation(
      async (locale: string) => ({
        locale,
        messages:
          locale === 'zh_Hans'
            ? { Account: '账号', Reports: '报表', Overview: '概览' }
            : { Account: 'Account', Reports: 'Reports', Overview: 'Overview' },
        catalog_revision: 1,
        digest: locale
      })
    );
    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([
      {
        id: 'account',
        title: 'Account',
        kind: 'group',
        placement: 'topbar',
        slug: 'route',
        children: [
          {
            id: 'reports',
            title: 'Reports',
            kind: 'group',
            placement: 'sidebar',
            children: [
              {
                id: 'overview',
                title: 'Overview',
                kind: 'page',
                placement: 'sidebar'
              },
              {
                id: 'custom',
                title: 'Custom page',
                kind: 'page',
                placement: 'sidebar'
              }
            ]
          }
        ]
      }
    ]);
    renderNavigation('/route/pages/overview');
    const navigation = await screen.findByRole('navigation', { name: 'Primary' });
    expect(
      await within(navigation).findByRole('link', { name: '账号' })
    ).toHaveAttribute('href', '/route');
    fireEvent.click(screen.getByRole('button', { name: '打开导航' }));
    const drawer = await screen.findByRole('dialog', { name: '1flowbase' });
    expect(await within(drawer).findByText('报表')).toBeInTheDocument();
    expect(
      await within(drawer).findByRole('link', { name: '概览' })
    ).toHaveAttribute('href', '/route/pages/overview');
    expect(
      within(drawer).getByRole('link', { name: 'Custom page' })
    ).toBeInTheDocument();
    await act(() => appI18n.changeLanguage('en_US'));
    expect(
      await within(navigation).findByRole('link', { name: 'Account' })
    ).toBeInTheDocument();
    expect(await within(drawer).findByText('Reports')).toBeInTheDocument();
    expect(
      await within(drawer).findByRole('link', { name: 'Overview' })
    ).toHaveAttribute('aria-current', 'page');
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
            id: 'group-reports',
            title: '报表',
            kind: 'group',
            placement: 'sidebar',
            children: [
              {
                id: 'page-sales',
                title: '销售看板',
                kind: 'page',
                placement: 'sidebar',
                content_presentation: 'tabs',
                children: []
              }
            ]
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
      within(nav)
        .getByRole('link', { name: '销售' })
        .closest('.ant-menu-submenu-title')
    ).toHaveAttribute('aria-haspopup', 'true');
    expect(within(nav).queryByRole('link', { name: '工作台' })).not.toBeInTheDocument();
    expect(within(nav).queryByRole('link', { name: '模板' })).not.toBeInTheDocument();
  });

  test('AC-001 and AC-002 nest each topbar page tree under its matching mobile topbar item', async () => {
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
        id: 'topbar-teacher',
        title: '教师',
        kind: 'group',
        placement: 'topbar',
        slug: 'teacher',
        children: [
          {
            id: 'group-materials',
            title: '教学材料',
            kind: 'group',
            placement: 'sidebar',
            children: [
              {
                id: 'page-lesson-one',
                title: '第一课',
                kind: 'page',
                placement: 'sidebar',
                children: []
              }
            ]
          }
        ]
      }
    ]);

    renderNavigation('/teacher/pages/page-lesson-one');

    const trigger = await screen.findByRole('button', { name: '打开导航' });
    fireEvent.click(trigger);

    const drawer = await screen.findByRole('dialog', { name: '1flowbase' });
    expect(within(drawer).getByText('1flowbase')).toBeInTheDocument();
    expect(within(drawer).queryByText('顶部栏目')).not.toBeInTheDocument();
    expect(within(drawer).getByRole('link', { name: '教师' })).toHaveAttribute(
      'href',
      '/teacher'
    );
    expect(within(drawer).queryByText('页面和分组')).not.toBeInTheDocument();
    const teacherSubmenu = within(drawer)
      .getByRole('link', { name: '教师' })
      .closest('.ant-menu-submenu') as HTMLElement | null;
    expect(teacherSubmenu).not.toBeNull();
    expect(within(teacherSubmenu!).getByText('教学材料')).toBeInTheDocument();
    const lessonLink = within(drawer).getByRole('link', { name: '第一课' });
    expect(lessonLink).toBeInTheDocument();
    expect(lessonLink.closest('.ant-menu-item')).toHaveClass(
      'ant-menu-item-selected'
    );
    lessonLink.addEventListener('click', (event) => event.preventDefault());
    fireEvent.click(lessonLink);
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '1flowbase' })
      ).not.toBeInTheDocument();
    });
  });

  test('AC-004 collects the topbar create action inside the mobile drawer', async () => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'actor-1',
        account: 'developer',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-123'
      },
      me: null
    });
    useFrontstageDesignModeStore.getState().setDesignMode(true);
    frontstageNavigationApi.fetchFrontstagePageTree.mockResolvedValue([]);

    renderNavigation('/templates');

    fireEvent.click(await screen.findByRole('button', { name: '打开导航' }));
    const drawer = await screen.findByRole('dialog', { name: '1flowbase' });
    expect(
      await within(drawer).findByRole('button', { name: '添加菜单' })
    ).toBeInTheDocument();
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
    const addMenuButton = await within(nav).findByRole('button', {
      name: '添加菜单'
    });
    expect(addMenuButton).toHaveClass(
      'frontstage-add-action-button',
      'frontstage-add-action-button--compact'
    );
    expect(addMenuButton).toHaveTextContent('添加菜单');
    expect(
      await within(nav).findByRole('link', { name: '新增菜单' })
    ).toHaveAttribute('href', '/new-space');
    expect(
      within(nav).queryByRole('button', { name: '管理顶部导航' })
    ).not.toBeInTheDocument();
    const topLevelItems = within(nav).getAllByRole('menuitem');
    expect(topLevelItems.map((item) => item.textContent)).toEqual([
      '子系统',
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

    expect(within(nav).queryByRole('link', { name: '工作台' })).not.toBeInTheDocument();
    expect(
      await within(nav).findByRole('link', { name: '子系统' })
    ).toBeInTheDocument();
    expect(within(nav).queryByRole('link', { name: '模板' })).not.toBeInTheDocument();
    expect(
      within(nav).queryByRole('link', { name: '设置' })
    ).not.toBeInTheDocument();
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
