import fs from 'node:fs';
import path from 'node:path';
import type { ReactElement } from 'react';

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
import { settingsSectionDefinitions } from '../../features/settings/lib/settings-sections';
import { appI18n } from '../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import { SettingsChromeMenu } from '../SettingsChromeMenu';
import { createSettingsChromeMenuItems } from '../settings-chrome-menu-items';

function isReactElementWithProps(
  value: unknown
): value is ReactElement<Record<string, unknown>> {
  return Boolean(
    value &&
    typeof value === 'object' &&
    'props' in value &&
    value.props &&
    typeof value.props === 'object'
  );
}

function getSettingsItem() {
  const items =
    createSettingsChromeMenuItems({
      pathname: '/settings/data-models',
      useRouterLinks: false,
      sections: settingsSectionDefinitions.map(({ key, label_key, to }) => ({
        key,
        label_key,
        to
      }))
    }) ?? [];

  return items[0];
}

function consoleNavigationForSettingsSections(sectionKeys: string[]) {
  const sections = sectionKeys.flatMap((sectionKey) => {
    const section = settingsSectionDefinitions.find(
      (definition) => definition.key === sectionKey
    );

    return section ? [section] : [];
  });

  return {
    route_definitions: sections.map((section) => ({
      route_id: `settings.${section.key}`,
      surface_key: section.key,
      path: section.to,
      surface_kind: 'system' as const
    })),
    navigation_items: sections.map((section, index) => ({
      item_id: section.key,
      route_id: `settings.${section.key}`,
      parent_item_id: 'settings',
      label_key: section.label_key,
      navigation_slot: 'settings' as const,
      order: index + 1
    })),
    permission_bindings: []
  };
}

describe('createSettingsChromeMenuItems', () => {
  beforeEach(() => {
    resetAuthStore();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockReset();
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForSettingsSections(
        settingsSectionDefinitions.map((section) => section.key)
      )
    );
  });

  test('renders visible labels for settings submenu links', async () => {
    await appI18n.changeLanguage('zh_Hans');
    const settingsItem = getSettingsItem();
    const children =
      settingsItem &&
      typeof settingsItem === 'object' &&
      'children' in settingsItem &&
      Array.isArray(settingsItem.children)
        ? settingsItem.children
        : [];

    const labels = children.flatMap((item) => {
      if (
        !item ||
        typeof item !== 'object' ||
        !('label' in item) ||
        !isReactElementWithProps(item.label)
      ) {
        return [];
      }

      return [item.label.props.children];
    });

    expect(labels).toContain('数据源');
    expect(labels).toContain('认证中心');
    expect(labels).not.toContain(undefined);
  });

  test('renders settings sections as the secondary chrome submenu', () => {
    const settingsItem = getSettingsItem();
    const children =
      settingsItem &&
      typeof settingsItem === 'object' &&
      'children' in settingsItem &&
      Array.isArray(settingsItem.children)
        ? settingsItem.children
        : [];

    expect(settingsItem).toMatchObject({
      key: 'settings',
      popupClassName: 'app-shell-settings-popup'
    });
    expect(children).toHaveLength(settingsSectionDefinitions.length);
    expect(
      children.map((item) =>
        typeof item === 'object' && item ? item.key : null
      )
    ).toEqual(settingsSectionDefinitions.map((section) => section.key));
    expect(
      children.some(
        (item) =>
          typeof item === 'object' &&
          item !== null &&
          'label' in item &&
          isReactElementWithProps(item.label) &&
          item.label.props['aria-current'] === 'page'
      )
    ).toBe(true);
  });

  test('renders the settings trigger as an accessible Ant icon', () => {
    const settingsItem = getSettingsItem();
    const label =
      settingsItem &&
      typeof settingsItem === 'object' &&
      'label' in settingsItem
        ? settingsItem.label
        : null;

    expect(isReactElementWithProps(label)).toBe(true);
    if (!isReactElementWithProps(label)) {
      throw new Error('Expected settings menu label to be a React element');
    }

    expect(label.props['aria-label']).toBe('设置');
    expect(label.props.children).toEqual(expect.anything());
    expect(label.props.children).not.toBe('设置');
  });

  test('constrains the settings dropdown to sixty percent viewport height', () => {
    const appShellCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../app-shell.css'),
      'utf8'
    );

    expect(appShellCss).toContain('.app-shell-settings-popup.ant-menu');
    expect(appShellCss).toContain('.app-shell-settings-popup .ant-menu');
    expect(appShellCss).toContain('max-height: 60vh;');
    expect(appShellCss).toContain('overflow-y: auto;');
  });

  test('keeps the settings trigger content-width without a fixed blank tail', () => {
    const appShellCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../app-shell.css'),
      'utf8'
    );
    const settingsBlockRule = appShellCss.match(
      /\.app-shell-settings-block \{([\s\S]*?)\n\}/
    )?.[1];

    expect(settingsBlockRule).not.toContain('min-width:');
  });

  test('SettingsChromeMenu shows only backend returned settings children', async () => {
    await appI18n.changeLanguage('zh_Hans');
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'root',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'root',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: ['api_reference.view.all', 'user.view.all']
      }
    });
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue(
      consoleNavigationForSettingsSections(['data-models', 'auth-center'])
    );

    render(
      <AppProviders>
        <SettingsChromeMenu
          pathname="/settings/data-models"
          useRouterLinks={false}
        />
      </AppProviders>
    );

    fireEvent.mouseEnter(await screen.findByLabelText('设置'));

    expect(await screen.findByText('数据源')).toBeInTheDocument();
    expect(screen.getByText('认证中心')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText('API 文档')).not.toBeInTheDocument();
    });
    expect(screen.queryByText('用户管理')).not.toBeInTheDocument();
  });

  test('SettingsChromeMenu keeps backend host extension settings children', async () => {
    await appI18n.changeLanguage('zh_Hans');
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'file-security.settings',
          surface_key: 'file-security.settings',
          path: '/settings/file-security',
          surface_kind: 'host_extension' as const
        }
      ],
      navigation_items: [
        {
          item_id: 'file-security.settings',
          route_id: 'file-security.settings',
          parent_item_id: 'settings',
          label_key: 'auto.api_documentation',
          navigation_slot: 'settings' as const,
          order: 1300
        }
      ],
      permission_bindings: []
    });

    render(
      <AppProviders>
        <SettingsChromeMenu
          pathname="/settings/file-security"
          useRouterLinks={false}
        />
      </AppProviders>
    );

    fireEvent.mouseEnter(await screen.findByLabelText('设置'));

    expect(await screen.findByText('API 文档')).toHaveAttribute(
      'href',
      '/settings/file-security'
    );
  });

  test('SettingsChromeMenu shows registry error instead of falling back to local settings sections', async () => {
    await appI18n.changeLanguage('zh_Hans');
    consoleNavigationApi.fetchSettingsConsoleNavigation.mockRejectedValue(
      new Error('registry unavailable')
    );

    render(
      <AppProviders>
        <SettingsChromeMenu
          pathname="/settings/data-models"
          useRouterLinks={false}
        />
      </AppProviders>
    );

    fireEvent.mouseEnter(await screen.findByLabelText('设置'));

    expect(await screen.findByText('控制台导航加载失败')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText('数据源')).not.toBeInTheDocument();
    });
    expect(screen.queryByText('API 文档')).not.toBeInTheDocument();
    expect(screen.queryByText('用户管理')).not.toBeInTheDocument();
  });
});
