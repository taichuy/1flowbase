import {
  createConsolePersonalAccessToken,
  listConsolePersonalAccessTokenRoleOptions,
  listConsolePersonalAccessTokens,
  revokeConsolePersonalAccessToken,
  type ConsolePersonalAccessToken,
  type ConsolePersonalAccessTokenRoleOption,
  type CreateConsolePersonalAccessTokenInput
} from '@1flowbase/api-client';

export type SettingsPersonalAccessToken = ConsolePersonalAccessToken;
export type SettingsPersonalAccessTokenRoleOption =
  ConsolePersonalAccessTokenRoleOption;
export type CreateSettingsPersonalAccessTokenInput =
  CreateConsolePersonalAccessTokenInput;

export const settingsPersonalAccessTokensQueryKey = [
  'settings',
  'personal-access-tokens'
] as const;
export const settingsPersonalAccessTokenRoleOptionsQueryKey = [
  'settings',
  'personal-access-tokens',
  'role-options'
] as const;

export async function fetchSettingsPersonalAccessTokens(): Promise<
  SettingsPersonalAccessToken[]
> {
  const response = await listConsolePersonalAccessTokens();

  return response.items;
}

export function createSettingsPersonalAccessToken(
  input: CreateSettingsPersonalAccessTokenInput,
  csrfToken: string
): Promise<SettingsPersonalAccessToken> {
  return createConsolePersonalAccessToken(input, csrfToken);
}

export async function fetchSettingsPersonalAccessTokenRoleOptions(): Promise<
  SettingsPersonalAccessTokenRoleOption[]
> {
  const response = await listConsolePersonalAccessTokenRoleOptions();

  return response.items;
}

export async function revokeSettingsPersonalAccessToken(
  apiKeyId: string,
  csrfToken: string
): Promise<void> {
  await revokeConsolePersonalAccessToken(apiKeyId, csrfToken);
}
