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

  test('keeps concise catalog toolbar keys aligned across locales', () => {
    expect(settingsZhHans.auto).toMatchObject({
      translation_catalog_filter: '筛选',
      translation_catalog_restore_defaults: '恢复默认值',
      new: '新增'
    });
    expect(settingsEnUS.auto).toMatchObject({
      translation_catalog_filter: 'Filter',
      translation_catalog_restore_defaults: 'Restore defaults',
      new: 'New'
    });
    expect(settingsZhHans.auto).not.toHaveProperty(
      'translation_catalog_apply_filters'
    );
    expect(settingsZhHans.auto).not.toHaveProperty(
      'translation_catalog_create_action'
    );
    expect(settingsEnUS.auto).not.toHaveProperty(
      'translation_catalog_apply_filters'
    );
    expect(settingsEnUS.auto).not.toHaveProperty(
      'translation_catalog_create_action'
    );
    expect(settingsZhHans.auto).not.toHaveProperty('translation_catalog_new');
    expect(settingsEnUS.auto).not.toHaveProperty('translation_catalog_new');
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
