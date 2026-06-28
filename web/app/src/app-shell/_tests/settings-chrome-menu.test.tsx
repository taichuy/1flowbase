import fs from 'node:fs';
import path from 'node:path';
import type { ReactElement } from 'react';

import { describe, expect, test } from 'vitest';

import { settingsSectionDefinitions } from '../../features/settings/lib/settings-sections';
import { appI18n } from '../../shared/i18n/app-i18n';
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
      isRoot: true,
      permissions: []
    }) ?? [];

  return items[0];
}

describe('createSettingsChromeMenuItems', () => {
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
});
