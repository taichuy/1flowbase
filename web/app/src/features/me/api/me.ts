import {
  authorizeConsoleExternalIdentity,
  fetchConsoleAuthenticationMethods,
  unlinkConsoleExternalIdentity,
  changeConsolePassword,
  fetchConsoleMe,
  updateConsoleMe,
  type ChangeConsolePasswordInput,
  type ConsoleMe,
  type UpdateConsoleMeInput
} from '@1flowbase/api-client';

export type MyProfile = ConsoleMe;
export type UpdateMyProfileInput = UpdateConsoleMeInput;
export type ChangeMyPasswordInput = ChangeConsolePasswordInput;

export const myProfileQueryKey = ['me', 'profile'] as const;

export function authorizeMyExternalIdentity(
  login_entry_id: string,
  csrfToken: string
) {
  return authorizeConsoleExternalIdentity({ login_entry_id }, csrfToken);
}

export function fetchMyProfile(): Promise<MyProfile> {
  return fetchConsoleMe();
}

export function updateMyProfile(
  input: UpdateMyProfileInput,
  csrfToken: string
): Promise<MyProfile> {
  return updateConsoleMe(input, csrfToken);
}

export function changeMyPassword(
  input: ChangeMyPasswordInput,
  csrfToken: string
): Promise<void> {
  return changeConsolePassword(input, csrfToken);
}

export type {
  ConsoleAuthenticationMethods,
  ConsoleAuthenticationProvider,
  ConsoleExternalIdentityBinding
} from '@1flowbase/api-client';

export const authenticationMethodsQueryKey = [
  'me',
  'authentication-methods'
] as const;

export function fetchMyAuthenticationMethods() {
  return fetchConsoleAuthenticationMethods();
}

export function unlinkMyExternalIdentity(
  identity_id: string,
  csrfToken: string
) {
  return unlinkConsoleExternalIdentity(identity_id, csrfToken);
}
