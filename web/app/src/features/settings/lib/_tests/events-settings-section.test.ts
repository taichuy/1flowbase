import { describe, expect, test } from 'vitest';
import {
  isSettingsSectionKey,
  settingsSectionItemsFromConsoleNavigation
} from '../settings-sections';

describe('backend event settings navigation consumer', () => {
  test('accepts the registered event surface and consumes its path/label without adding client permissions', () => {
    expect(isSettingsSectionKey('events')).toBe(true);
    expect(
      settingsSectionItemsFromConsoleNavigation({
        route_definitions: [
          {
            route_id: 'settings.events',
            surface_key: 'events',
            path: '/settings/events'
          }
        ],
        navigation_items: [
          {
            route_id: 'settings.events',
            parent_item_id: 'settings',
            label_key: 'events.title',
            navigation_slot: 'settings',
            order: 30
          }
        ]
      })
    ).toEqual([
      {
        route_id: 'settings.events',
        key: 'events',
        label_key: 'events.title',
        to: '/settings/events'
      }
    ]);
    expect(
      settingsSectionItemsFromConsoleNavigation({
        route_definitions: [],
        navigation_items: []
      })
    ).toEqual([]);
  });
});
