import {
  copyConsoleAuthCenterAuthenticator,
  createConsoleAuthCenterAuthenticator,
  deleteConsoleAuthCenterAuthenticator,
  enableConsoleAuthCenterAuthenticator,
  fetchConsoleAuthCenterOverview,
  reorderConsoleAuthCenterAuthenticators,
  type ConsoleAuthCenterCopyAuthenticatorInput,
  type ConsoleAuthCenterCreateAuthenticatorInput,
  updateConsoleAuthCenterAuthenticatorConfig,
  type ConsoleAuthCenterAuthenticatorConfigInput,
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

export function enableSettingsAuthCenterAuthenticator(
  authenticatorId: string,
  csrfToken: string
) {
  return enableConsoleAuthCenterAuthenticator(authenticatorId, csrfToken);
}

export function createSettingsAuthCenterAuthenticator(
  input: ConsoleAuthCenterCreateAuthenticatorInput,
  csrfToken: string
) {
  return createConsoleAuthCenterAuthenticator(input, csrfToken);
}

export function copySettingsAuthCenterAuthenticator(
  sourceId: string,
  input: ConsoleAuthCenterCopyAuthenticatorInput,
  csrfToken: string
) {
  return copyConsoleAuthCenterAuthenticator(
    sourceId,
    input,
    csrfToken
  );
}

export function deleteSettingsAuthCenterAuthenticator(
  authenticatorId: string,
  csrfToken: string
) {
  return deleteConsoleAuthCenterAuthenticator(authenticatorId, csrfToken);
}

export function reorderSettingsAuthCenterAuthenticators(
  ids: string[],
  csrfToken: string
) {
  return reorderConsoleAuthCenterAuthenticators({ ids }, csrfToken);
}

export function updateSettingsAuthCenterAuthenticatorConfig(
  authenticatorId: string,
  input: ConsoleAuthCenterAuthenticatorConfigInput,
  csrfToken: string
) {
  return updateConsoleAuthCenterAuthenticatorConfig(
    authenticatorId,
    input,
    csrfToken
  );
}
