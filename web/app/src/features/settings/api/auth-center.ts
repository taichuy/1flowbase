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
  authenticatorName: string,
  csrfToken: string
) {
  return enableConsoleAuthCenterAuthenticator(authenticatorName, csrfToken);
}

export function createSettingsAuthCenterAuthenticator(
  input: ConsoleAuthCenterCreateAuthenticatorInput,
  csrfToken: string
) {
  return createConsoleAuthCenterAuthenticator(input, csrfToken);
}

export function copySettingsAuthCenterAuthenticator(
  authenticatorName: string,
  input: ConsoleAuthCenterCopyAuthenticatorInput,
  csrfToken: string
) {
  return copyConsoleAuthCenterAuthenticator(
    authenticatorName,
    input,
    csrfToken
  );
}

export function deleteSettingsAuthCenterAuthenticator(
  authenticatorName: string,
  csrfToken: string
) {
  return deleteConsoleAuthCenterAuthenticator(authenticatorName, csrfToken);
}

export function reorderSettingsAuthCenterAuthenticators(
  names: string[],
  csrfToken: string
) {
  return reorderConsoleAuthCenterAuthenticators({ names }, csrfToken);
}

export function updateSettingsAuthCenterAuthenticatorConfig(
  authenticatorName: string,
  input: ConsoleAuthCenterAuthenticatorConfigInput,
  csrfToken: string
) {
  return updateConsoleAuthCenterAuthenticatorConfig(
    authenticatorName,
    input,
    csrfToken
  );
}
