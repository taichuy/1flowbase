import { apiFetch, apiFetchVoid } from '../../transport';
export interface ConsoleDepartment {
  id: string;
  name: string;
  parent_id: string | null;
  role_codes: string[];
  member_count: number;
}
export interface DepartmentInput {
  name: string;
  parent_id: string | null;
  role_codes: string[];
}
export interface MemberDepartmentsInput {
  department_ids: string[];
  primary_department_id: string | null;
}
const path = '/api/console/settings/departments';
export const listConsoleDepartments = () =>
  apiFetch<ConsoleDepartment[]>({ path });
export const createConsoleDepartment = (
  body: DepartmentInput,
  csrfToken: string
) => apiFetch<ConsoleDepartment>({ path, method: 'POST', body, csrfToken });
export const updateConsoleDepartment = (
  id: string,
  body: DepartmentInput,
  csrfToken: string
) =>
  apiFetch<ConsoleDepartment>({
    path: `${path}/${encodeURIComponent(id)}`,
    method: 'PATCH',
    body,
    csrfToken
  });
export const deleteConsoleDepartment = (id: string, csrfToken: string) =>
  apiFetchVoid({
    path: `${path}/${encodeURIComponent(id)}`,
    method: 'DELETE',
    csrfToken
  });
export const replaceConsoleMemberDepartments = (
  id: string,
  body: MemberDepartmentsInput,
  csrfToken: string
) =>
  apiFetchVoid({
    path: `/api/console/settings/members/${encodeURIComponent(id)}/departments`,
    method: 'PUT',
    body,
    csrfToken
  });

export interface DepartmentAccess {
  can_list: boolean;
  can_create: boolean;
  can_update: boolean;
  can_delete: boolean;
  can_replace_member_departments: boolean;
  can_assign_roles: boolean;
}
export const getConsoleDepartmentAccess = () =>
  apiFetch<DepartmentAccess>({ path: `${path}/access` });
export interface MemberRoleOption {
  code: string;
  name: string;
}
export const listConsoleMemberRoleOptions = () =>
  apiFetch<MemberRoleOption[]>({
    path: '/api/console/settings/members/role-options'
  });
