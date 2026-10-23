import {
  listConsoleMembers,
  type ChangeConsolePasswordInput,
  type ConsoleMember,
  type CreateConsoleMemberInput,
  type UpdateConsoleMemberInput
} from '@1flowbase/api-client';

export {
  createConsoleMember as createSettingsMember,
  updateConsoleMember as updateSettingsMember,
  disableConsoleMember as disableSettingsMember,
  enableConsoleMember as enableSettingsMember,
  deleteConsoleMember as deleteSettingsMember,
  resetConsoleMemberPassword as resetSettingsMemberPassword,
  changeConsolePassword as changeCurrentUserPassword,
  replaceConsoleMemberRoles as replaceSettingsMemberRoles
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
