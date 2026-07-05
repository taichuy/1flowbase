import { render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const consoleNavigationApi = vi.hoisted(() => ({
  settingsConsoleNavigationQueryKey: ['settings', 'console-navigation'],
  fetchSettingsConsoleNavigation: vi.fn()
}));

vi.mock(
  '../../features/settings/api/console-navigation',
  () => consoleNavigationApi
);

import { AppProviders } from '../../app/AppProviders';
import { Navigation } from '../Navigation';
import { appI18n } from '../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';

const primaryRouteRecords = {
  home: {
    path: '/',
    label_key: 'auto.workbench'
  },
  frontstage: {
    path: '/frontstage',
    label_key: 'auto.frontstage'
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
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForPrimaryRoutes([
        'home',
        'frontstage',
        'embedded-apps',
        'templates'
      ])
    );
  });

  test('links 前台 to base frontstage path when workspace is available', async () => {
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
        permissions: ['route_page.view.all']
      }
    });

    renderNavigation('/embedded-apps');

    expect(await screen.findByRole('link', { name: '前台' })).toHaveAttribute(
      'href',
      '/frontstage'
    );
  });

  test('links 前台 to base frontstage path when workspace is not available', async () => {
    resetAuthStore();

    renderNavigation('/embedded-apps');

    expect(await screen.findByRole('link', { name: '前台' })).toHaveAttribute(
      'href',
      '/frontstage'
    );
  });

  test('renders primary console navigation and keeps settings out of the primary rail', async () => {
    resetAuthStore();

    renderNavigation('/embedded-apps');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });

    expect(
      await within(nav).findByRole('link', { name: '工作台' })
    ).toBeInTheDocument();
    expect(within(nav).getByRole('link', { name: '前台' })).toBeInTheDocument();
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
        permissions: [
          'route_page.view.all',
          'embedded_app.view.all',
          'template.view.all'
        ]
      }
    });
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForPrimaryRoutes(['frontstage', 'embedded-apps'])
    );

    renderNavigation('/embedded-apps');

    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    await waitFor(() => {
      expect(
        within(nav).queryByRole('link', { name: '工作台' })
      ).not.toBeInTheDocument();
    });
    expect(within(nav).getByRole('link', { name: '前台' })).toHaveAttribute(
      'href',
      '/frontstage'
    );
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
      within(nav).queryByRole('link', { name: '前台' })
    ).not.toBeInTheDocument();
    expect(
      within(nav).queryByRole('link', { name: '子系统' })
    ).not.toBeInTheDocument();
    expect(
      within(nav).queryByRole('link', { name: '模板' })
    ).not.toBeInTheDocument();
  });
});
