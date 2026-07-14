import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleRole,
  deleteConsoleRole,
  fetchConsoleRoleDataPolicy,
  fetchConsoleRoleFrontstageRoutes,
  fetchConsoleRolePermissions,
  listConsoleRoles,
  replaceConsoleRoleDataPolicy,
  replaceConsoleRoleFrontstageRoutes,
  replaceConsoleRolePermissions,
  updateConsoleRole,
  type ReplaceConsoleRoleDataPolicyInput
} from '../console-roles';

describe('console roles client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );

  test('uses the Settings namespace for every role-management route (Issue #1256 AC-003)', async () => {
    const role = {
      code: 'member',
      name: 'Member',
      introduction: 'Workspace member'
    };

    await expect(listConsoleRoles()).resolves.toMatchObject({
      path: '/api/console/settings/roles'
    });
    await expect(createConsoleRole(role, 'csrf-123')).resolves.toMatchObject({
      path: '/api/console/settings/roles',
      method: 'POST'
    });
    await expect(
      updateConsoleRole('member', role, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member',
      method: 'PATCH'
    });
    await expect(
      deleteConsoleRole('member', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member',
      method: 'DELETE'
    });
    await expect(fetchConsoleRolePermissions('member')).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/permissions'
    });
    await expect(
      replaceConsoleRolePermissions(
        'member',
        { permission_codes: [] },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/permissions',
      method: 'PUT'
    });
    await expect(
      fetchConsoleRoleFrontstageRoutes('member')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/frontstage-routes'
    });
    await expect(
      replaceConsoleRoleFrontstageRoutes(
        'member',
        { page_ids: [], tab_ids: [] },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/frontstage-routes',
      method: 'PUT'
    });
    await expect(fetchConsoleRoleDataPolicy('member')).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/data-policy'
    });
  });

  test('replaces role data policy with backend field names', async () => {
    const input: ReplaceConsoleRoleDataPolicyInput = {
      default_policy: {
        can_view: true,
        can_create: false,
        can_update: true,
        can_delete: false,
        default_view_scope: 'own',
        default_update_scope: 'scope_all',
        default_delete_scope: 'own'
      },
      model_policies: [
        {
          data_model_id: 'model-orders',
          can_create_override: false,
          view_scope_override: null,
          update_scope_override: 'own',
          delete_scope_override: 'scope_all'
        }
      ]
    };

    await expect(
      replaceConsoleRoleDataPolicy('member', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/data-policy',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: input
    });
  });
});
