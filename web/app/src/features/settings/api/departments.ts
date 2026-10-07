export {
  getConsoleDepartmentAccess as fetchSettingsDepartmentAccess,
  listConsoleMemberRoleOptions as fetchSettingsMemberRoleOptions,
  type DepartmentAccess,
  listConsoleDepartments as fetchSettingsDepartments,
  createConsoleDepartment as createSettingsDepartment,
  updateConsoleDepartment as updateSettingsDepartment,
  deleteConsoleDepartment as deleteSettingsDepartment,
  replaceConsoleMemberDepartments as replaceSettingsMemberDepartments,
  type ConsoleDepartment as SettingsDepartment,
  type DepartmentTreeItem,
  type DepartmentPage,
  type DepartmentListParams,
  type DepartmentInput,
  type MemberDepartmentsInput
} from '@1flowbase/api-client';
export const settingsDepartmentsQueryKey = ['settings', 'departments'] as const;
export const settingsDepartmentAccessQueryKey = [
  'settings',
  'departments',
  'access'
] as const;
export const settingsMemberRoleOptionsQueryKey = [
  'settings',
  'members',
  'role-options'
] as const;
