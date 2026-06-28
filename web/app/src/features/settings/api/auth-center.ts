import {
  fetchConsoleAuthCenterOverview,
  type ConsoleAuthCenterOverview
} from '@1flowbase/api-client';

export type SettingsAuthCenterOverview = ConsoleAuthCenterOverview;

export const settingsAuthCenterOverviewQueryKey = [
  'settings',
  'auth-center',
  'overview'
] as const;

export function fetchSettingsAuthCenterOverview() {
  return fetchConsoleAuthCenterOverview();
}
