import {
  authorizeConsoleExternalIdentity,
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
