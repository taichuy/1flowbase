import { describe, test, expect } from 'vitest';
import {
  authenticateWithPermissions,
  consoleNavigationApi,
  docsApi,
  renderApp,
  settingsConsoleNavigation
} from './settings-page.fixtures';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { resetAuthStore } from '../../../../../state/auth-store';

describe('SettingsPage', () => {
  test('uses backend settings registry order instead of permissions for /settings redirects', async () => {
    authenticateWithPermissions([]);
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['docs', 'communications', 'members'])
    );
    const view = renderApp('/settings');

    await waitFor(
      () => {
        expect(window.location.pathname).toBe('/settings/docs');
      },
      { timeout: 5000 }
    );
    await waitFor(() => {
      expect(docsApi.fetchSettingsApiDocsCatalog).toHaveBeenCalled();
    });
    view.unmount();

    resetAuthStore();
    docsApi.fetchSettingsApiDocsCatalog.mockClear();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['data-models', 'members'])
    );
    authenticateWithPermissions([
      'settings_feature.access.system.docs',
      'user.view.all'
    ]);
    renderApp('/settings');

    await waitFor(
      () => {
        expect(window.location.pathname).toBe('/settings/data-models');
      },
      { timeout: 5000 }
    );
    expect(docsApi.fetchSettingsApiDocsCatalog).not.toHaveBeenCalled();
  }, 10000);

  test('redirects a missing requested settings section to the first backend registry section', async () => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      settingsConsoleNavigation(['data-models', 'members'])
    );
    authenticateWithPermissions([
      'settings_feature.access.system.docs',
      'user.view.all'
    ]);

    renderApp('/settings/docs');

    await waitFor(
      () => {
        expect(window.location.pathname).toBe('/settings/data-models');
      },
      { timeout: 5000 }
    );
    expect(docsApi.fetchSettingsApiDocsCatalog).not.toHaveBeenCalled();
  }, 10000);

  test('shows registry error instead of falling back to local settings sections', async () => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockRejectedValue(
      new Error('registry unavailable')
    );
    authenticateWithPermissions(['settings_feature.access.system.docs']);

    renderApp('/settings/docs');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/docs');
    });
    expect(await screen.findByText('设置导航加载失败')).toBeInTheDocument();
    expect(docsApi.fetchSettingsApiDocsCatalog).not.toHaveBeenCalled();
  }, 10000);

  test('shows registry loading instead of an empty settings state while navigation is pending', async () => {
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReturnValue(
      new Promise(() => undefined)
    );
    authenticateWithPermissions(['settings_feature.access.system.docs']);

    renderApp('/settings/docs');

    expect(await screen.findByText('加载中...')).toBeInTheDocument();
    expect(
      screen.queryByText('当前账号暂无可访问内容')
    ).not.toBeInTheDocument();
    expect(docsApi.fetchSettingsApiDocsCatalog).not.toHaveBeenCalled();
  });

  test('renders /settings/members when user.view.all is present', async () => {
    authenticateWithPermissions(['user.view.all']);

    renderApp('/settings/members');

    await waitFor(() => {
      expect(window.location.pathname).toBe('/settings/members');
    });
    await waitFor(() => {
      expect(screen.getByTestId('section-page-layout')).toHaveClass(
        'section-page-layout--wide',
        'section-page-layout--viewport'
      );
    });
    expect(
      await screen.findByText(
        '重置密码会将目标账号密码重置为默认临时密码，并要求用户登录后立即修改。'
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '新建用户' })
    ).not.toBeInTheDocument();
  });

  test('keeps the selected members tab in the URL', async () => {
    authenticateWithPermissions(['user.view.all']);

    renderApp('/settings/members');

    const creditsTab = await screen.findByRole('tab', { name: '用户余额' });
    fireEvent.click(creditsTab);

    await waitFor(() => {
      expect(window.location.search).toBe('?tabs=credits');
      expect(creditsTab).toHaveAttribute('aria-selected', 'true');
    });

    fireEvent.click(screen.getByRole('tab', { name: '用户管理' }));
    await waitFor(() => {
      expect(window.location.search).toBe('?tabs=members');
    });
  });

  test('restores the credits tab from the URL', async () => {
    authenticateWithPermissions(['user.view.all']);

    renderApp('/settings/members?tabs=credits');

    expect(
      await screen.findByRole('tab', { name: '用户余额' })
    ).toHaveAttribute('aria-selected', 'true');
  });
});
