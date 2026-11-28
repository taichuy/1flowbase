import { fireEvent, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import {
  authenticateWithPermissions,
  departmentsApi,
  membersApi,
  renderApp
} from './settings-sections.fixtures';

const imports = vi.hoisted(() => ({ department: vi.fn(), credit: vi.fn() }));
vi.mock('../../components/organization/DepartmentManagementPanel', () => {
  imports.department();
  return { DepartmentManagementPanel: () => <h2>Department panel</h2> };
});
vi.mock('../../components/billing/CreditManagementPanel', () => {
  imports.credit();
  return { CreditManagementPanel: () => <h2>Credit panel</h2> };
});

const noDepartmentAccess = {
  can_list: false,
  can_create: false,
  can_update: false,
  can_delete: false,
  can_assign_roles: false,
  can_replace_member_departments: false
};

test('only loads the selected Member tab and keeps URL and department permission checks', async () => {
  authenticateWithPermissions(['user.view.all', 'user.manage.all'], 'root');
  departmentsApi.fetchSettingsDepartmentAccess.mockResolvedValue(
    noDepartmentAccess
  );
  renderApp('/settings/members?tabs=members');

  await waitFor(() => {
    expect(membersApi.fetchSettingsMembers).toHaveBeenCalledTimes(1);
    expect(departmentsApi.fetchSettingsDepartmentAccess).toHaveBeenCalledTimes(
      1
    );
  });
  expect(departmentsApi.fetchSettingsDepartments).not.toHaveBeenCalled();
  expect(imports.department).not.toHaveBeenCalled();
  expect(imports.credit).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole('tab', { name: '用户余额' }));
  expect(
    await screen.findByRole('heading', { name: 'Credit panel' })
  ).toBeVisible();
  expect(window.location.search).toBe('?tabs=credits');
  expect(imports.department).not.toHaveBeenCalled();
  expect(membersApi.fetchSettingsMembers).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole('tab', { name: '组织管理' }));
  expect(
    await screen.findByRole('heading', { name: 'Department panel' })
  ).toBeVisible();
  expect(window.location.search).toBe('?tabs=departments');
  expect(
    screen.queryByRole('heading', { name: 'Credit panel', hidden: true })
  ).not.toBeInTheDocument();
});
