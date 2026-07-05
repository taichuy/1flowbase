import { describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  getConsoleNavigation: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();

  return {
    ...actual,
    getConsoleNavigation: apiClient.getConsoleNavigation
  };
});

import {
  fetchSettingsConsoleNavigation,
  settingsConsoleNavigationQueryKey
} from '../console-navigation';

describe('settings console navigation API', () => {
  test('keeps a stable query key for the console navigation registry', () => {
    expect(settingsConsoleNavigationQueryKey).toEqual([
      'settings',
      'console-navigation'
    ]);
  });

  test('delegates to the api client console navigation endpoint', async () => {
    apiClient.getConsoleNavigation.mockResolvedValue({
      route_definitions: [],
      navigation_items: [],
      permission_bindings: []
    });

    await expect(fetchSettingsConsoleNavigation()).resolves.toMatchObject({
      route_definitions: [],
      navigation_items: [],
      permission_bindings: []
    });
    expect(apiClient.getConsoleNavigation).toHaveBeenCalledWith();
  });
});
