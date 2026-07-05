import {
  getConsoleNavigation,
  type ConsoleNavigation
} from '@1flowbase/api-client';

export type SettingsConsoleNavigation = ConsoleNavigation;

export const settingsConsoleNavigationQueryKey = [
  'settings',
  'console-navigation'
] as const;

export function fetchSettingsConsoleNavigation(): Promise<SettingsConsoleNavigation> {
  return getConsoleNavigation();
}
