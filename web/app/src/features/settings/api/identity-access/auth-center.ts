import {
  fetchConsoleAuthCenterOverview,
  reorderConsoleAuthCenterLoginEntries,
  type ConsoleAuthCenterOverview
} from '@1flowbase/api-client';

export {
  updateConsoleAuthCenterLoginEntryEnabled as updateSettingsAuthCenterLoginEntryEnabled,
  createConsoleAuthCenterLoginEntry as createSettingsAuthCenterLoginEntry,
  copyConsoleAuthCenterLoginEntry as copySettingsAuthCenterLoginEntry,
  deleteConsoleAuthCenterLoginEntry as deleteSettingsAuthCenterLoginEntry,
  updateConsoleAuthCenterLoginEntryConfig as updateSettingsAuthCenterLoginEntryConfig,
  updateConsoleAuthCenterLoginEntryPublicUiBlock as updateSettingsAuthCenterLoginEntryPublicUiBlock
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

export function reorderSettingsAuthCenterLoginEntries(
  ids: string[],
  csrfToken: string
) {
  return reorderConsoleAuthCenterLoginEntries({ ids }, csrfToken);
}
