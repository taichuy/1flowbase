import {
  enableConsoleAuthCenterAuthenticator,
  fetchConsoleAuthCenterOverview,
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
