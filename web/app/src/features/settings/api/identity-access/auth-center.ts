import {
  copyConsoleAuthCenterLoginEntry,
  createConsoleAuthCenterLoginEntry,
  deleteConsoleAuthCenterLoginEntry,
  fetchConsoleAuthCenterOverview,
  reorderConsoleAuthCenterLoginEntries,
  type ConsoleAuthCenterCopyLoginEntryInput,
  type ConsoleAuthCenterCreateLoginEntryInput,
  type ConsoleAuthCenterLoginEntryEnabledInput,
  updateConsoleAuthCenterLoginEntryEnabled,
  updateConsoleAuthCenterLoginEntryConfig,
  updateConsoleAuthCenterLoginEntryPublicUiBlock,
  type ConsoleAuthCenterLoginEntryConfigInput,
  type ConsoleAuthCenterLoginEntryPublicUiBlockInput,
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

export function updateSettingsAuthCenterLoginEntryEnabled(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryEnabledInput,
  csrfToken: string
) {
  return updateConsoleAuthCenterLoginEntryEnabled(
    loginEntryId,
    input,
    csrfToken
  );
}

export function createSettingsAuthCenterLoginEntry(
  input: ConsoleAuthCenterCreateLoginEntryInput,
  csrfToken: string
) {
  return createConsoleAuthCenterLoginEntry(input, csrfToken);
}

export function copySettingsAuthCenterLoginEntry(
  sourceId: string,
  input: ConsoleAuthCenterCopyLoginEntryInput,
  csrfToken: string
) {
  return copyConsoleAuthCenterLoginEntry(sourceId, input, csrfToken);
}

export function deleteSettingsAuthCenterLoginEntry(
  loginEntryId: string,
  csrfToken: string
) {
  return deleteConsoleAuthCenterLoginEntry(loginEntryId, csrfToken);
}

export function reorderSettingsAuthCenterLoginEntries(
  ids: string[],
  csrfToken: string
) {
  return reorderConsoleAuthCenterLoginEntries({ ids }, csrfToken);
}

export function updateSettingsAuthCenterLoginEntryConfig(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryConfigInput,
  csrfToken: string
) {
  return updateConsoleAuthCenterLoginEntryConfig(
    loginEntryId,
    input,
    csrfToken
  );
}

export function updateSettingsAuthCenterLoginEntryPublicUiBlock(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryPublicUiBlockInput,
  csrfToken: string
) {
  return updateConsoleAuthCenterLoginEntryPublicUiBlock(
    loginEntryId,
    input,
    csrfToken
  );
}
