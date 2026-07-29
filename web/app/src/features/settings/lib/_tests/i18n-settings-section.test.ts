import { describe, expect, test } from 'vitest';

import {
  isSettingsSectionKey,
  settingsSectionDefinitions,
  settingsSectionItemsFromConsoleNavigation
} from '../settings-sections';
import settingsEnUS from '../../i18n/en_US.json';
import settingsZhHans from '../../i18n/zh_Hans.json';

describe('multilingual settings section', () => {
  test('AC-001 uses mature user-facing names while preserving the catalog key', () => {
    expect(settingsZhHans.auto.translation_catalog_title).toBe('多语言');
    expect(settingsEnUS.auto.translation_catalog_title).toBe('Languages');
  });

  test('AC-007 keeps the explicit route and selected section identity aligned', () => {
    expect(isSettingsSectionKey('i18n')).toBe(true);
    expect(settingsSectionDefinitions).toContainEqual({
      key: 'i18n',
      label_key: 'auto.translation_catalog_title',
      to: '/settings/i18n'
    });

    expect(
      settingsSectionItemsFromConsoleNavigation({
        route_definitions: [
          {
            route_id: 'settings.i18n',
            surface_key: 'i18n',
            path: '/settings/i18n'
          }
        ],
        navigation_items: [
          {
            route_id: 'settings.i18n',
            parent_item_id: 'settings',
            label_key: 'auto.translation_catalog_title',
            navigation_slot: 'settings',
            order: 1
          }
        ]
      })
    ).toEqual([
      {
        key: 'i18n',
        label_key: 'auto.translation_catalog_title',
        to: '/settings/i18n'
      }
    ]);
  });
});
