import { describe, expect, test, vi } from 'vitest';
vi.mock('../../../transport', () => ({
  apiFetch: vi.fn(async (input) => input),
  apiFetchVoid: vi.fn(async (input) => input)
}));
import {
  createConsoleDepartment,
  deleteConsoleDepartment,
  getConsoleDepartmentAccess,
  listConsoleDepartments,
  replaceConsoleMemberDepartments,
  updateConsoleDepartment
} from '../index';
import { listConsoleMembers } from '../../../console-members';
describe('department wire contract', () => {
  test('reads official access independently of the department list', async () => {
    await expect(getConsoleDepartmentAccess()).resolves.toMatchObject({
      path: '/api/console/settings/departments/access'
    });
    await expect(listConsoleDepartments()).resolves.toMatchObject({
      path: '/api/console/settings/departments'
    });
  });
  test('serializes direct children, search pages and selected IDs using the backend field names', async () => {
    await expect(
      listConsoleDepartments({ parent_id: 'parent', limit: 2, cursor: 'a+/=' })
    ).resolves.toMatchObject({
      path: '/api/console/settings/departments?parent_id=parent&limit=2&cursor=a%2B%2F%3D'
    });
    await expect(
      listConsoleDepartments({ prefix: 'Dev', cursor: 'next' })
    ).resolves.toMatchObject({
      path: '/api/console/settings/departments?prefix=Dev&cursor=next'
    });
    await expect(listConsoleDepartments({ ids: 'a,b' })).resolves.toMatchObject(
      { path: '/api/console/settings/departments?ids=a%2Cb' }
    );
  });
  test('writes parent_id and role_codes without aliases', async () => {
    const body = {
      name: 'Engineering',
      parent_id: null,
      role_codes: ['operator']
    };
    await expect(createConsoleDepartment(body, 'csrf')).resolves.toMatchObject({
      method: 'POST',
      body,
      csrfToken: 'csrf'
    });
    await expect(
      updateConsoleDepartment('d1', body, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/departments/d1',
      method: 'PATCH',
      body
    });
    await expect(deleteConsoleDepartment('d1', 'csrf')).resolves.toMatchObject({
      method: 'DELETE',
      csrfToken: 'csrf'
    });
  });
  test('requests server subtree members and replaces assignments explicitly', async () => {
    await expect(listConsoleMembers(undefined, 'd1')).resolves.toMatchObject({
      path: '/api/console/settings/members?department_id=d1'
    });
    await expect(listConsoleMembers()).resolves.toMatchObject({
      path: '/api/console/settings/members'
    });
    await expect(
      replaceConsoleMemberDepartments(
        'u1',
        { department_ids: ['d1'], primary_department_id: 'd1' },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/members/u1/departments',
      method: 'PUT',
      body: { department_ids: ['d1'], primary_department_id: 'd1' }
    });
  });
});
