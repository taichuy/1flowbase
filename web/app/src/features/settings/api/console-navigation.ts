import { queryOptions } from '@tanstack/react-query';
import { navigationQueryStaleTime } from '../../../state/navigation-query-scope';

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

export function settingsConsoleNavigationQueryOptions(scope: string) {
  return queryOptions({
    queryKey: [...settingsConsoleNavigationQueryKey, scope],
    queryFn: fetchSettingsConsoleNavigation,
    staleTime: navigationQueryStaleTime,
    retry: false
  });
}
