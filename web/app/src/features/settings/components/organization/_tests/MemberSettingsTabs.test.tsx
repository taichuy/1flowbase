import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  access: vi.fn(),
  tab: 'members'
}));
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mocks.navigate,
  useRouterState: () => mocks.tab
}));
vi.mock('../../../api/departments', () => ({
  fetchSettingsDepartmentAccess: mocks.access,
  settingsDepartmentAccessQueryKey: ['settings', 'departments', 'access']
}));
vi.mock('../../MemberManagementPanel', () => ({
  MemberManagementPanel: () => <div>Members content</div>
}));
vi.mock('../../billing/CreditManagementPanel', () => ({
  CreditManagementPanel: () => <div>Credits content</div>
}));
vi.mock('../DepartmentManagementPanel', () => ({
  DepartmentManagementPanel: () => <div>Departments content</div>
}));
import { AppProviders } from '../../../../../app/AppProviders';
import { MemberSettingsTabs } from '../../../pages/settings-page/MemberSettingsTabs';
describe('member settings tabs', () => {
  beforeEach(() => {
    mocks.tab = 'members';
    mocks.access.mockResolvedValue({
      can_list: true,
      can_create: false,
      can_update: false,
      can_delete: false,
      can_assign_roles: false,
      can_replace_member_departments: false
    });
  });
  test('orders users, organizations and credits and navigates with the departments query value', async () => {
    render(
      <AppProviders>
        <MemberSettingsTabs canManageMembers />
      </AppProviders>
    );
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '用户管理',
      '组织管理',
      '用户余额'
    ]);
    fireEvent.click(screen.getByRole('tab', { name: '组织管理' }));
    await waitFor(() =>
      expect(mocks.navigate).toHaveBeenCalledWith({
        to: '/settings/members',
        search: { tabs: 'departments' }
      })
    );
  });
  test('restores the departments tab from URL query state', async () => {
    mocks.tab = 'departments';
    render(
      <AppProviders>
        <MemberSettingsTabs canManageMembers={false} />
      </AppProviders>
    );
    expect(await screen.findByText('Departments content')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '组织管理' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
  });
});
