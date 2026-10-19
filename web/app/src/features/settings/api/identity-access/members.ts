import {
  changeConsolePassword,
  createConsoleMember,
  deleteConsoleMember,
  disableConsoleMember,
  enableConsoleMember,
  listConsoleMembers,
  replaceConsoleMemberRoles,
  resetConsoleMemberPassword,
  updateConsoleMember,
  type ChangeConsolePasswordInput,
  type ConsoleMember,
  type CreateConsoleMemberInput,
  type ReplaceConsoleMemberRolesInput,
  type ResetConsoleMemberPasswordInput,
  type UpdateConsoleMemberInput
} from '@1flowbase/api-client';

export type SettingsMember = ConsoleMember;
export type CreateSettingsMemberInput = CreateConsoleMemberInput;
export type UpdateSettingsMemberInput = UpdateConsoleMemberInput;
export type ChangeCurrentUserPasswordInput = ChangeConsolePasswordInput;

export const settingsMembersQueryKey = ['settings', 'members'] as const;

export function fetchSettingsMembers(
  department_id?: string
): Promise<SettingsMember[]> {
  return listConsoleMembers(undefined, department_id);
}

export function createSettingsMember(
  input: CreateSettingsMemberInput,
  csrfToken: string
): Promise<SettingsMember> {
  return createConsoleMember(input, csrfToken);
}

export function updateSettingsMember(
  memberId: string,
  input: UpdateSettingsMemberInput,
  csrfToken: string
): Promise<SettingsMember> {
  return updateConsoleMember(memberId, input, csrfToken);
}

export function disableSettingsMember(
  memberId: string,
  csrfToken: string
): Promise<void> {
  return disableConsoleMember(memberId, csrfToken);
}

export function enableSettingsMember(
  memberId: string,
  csrfToken: string
): Promise<void> {
  return enableConsoleMember(memberId, csrfToken);
}

export function deleteSettingsMember(
  memberId: string,
  csrfToken: string
): Promise<void> {
  return deleteConsoleMember(memberId, csrfToken);
}

export function resetSettingsMemberPassword(
  memberId: string,
  input: ResetConsoleMemberPasswordInput,
  csrfToken: string
): Promise<void> {
  return resetConsoleMemberPassword(memberId, input, csrfToken);
}

export function changeCurrentUserPassword(
  input: ChangeCurrentUserPasswordInput,
  csrfToken: string
): Promise<void> {
  return changeConsolePassword(input, csrfToken);
}

export function replaceSettingsMemberRoles(
  memberId: string,
  input: ReplaceConsoleMemberRolesInput,
  csrfToken: string
): Promise<void> {
  return replaceConsoleMemberRoles(memberId, input, csrfToken);
}
