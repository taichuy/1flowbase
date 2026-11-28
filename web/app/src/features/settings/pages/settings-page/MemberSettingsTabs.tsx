import { lazy, Suspense } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Spin, Tabs } from 'antd';
import {
  fetchSettingsDepartmentAccess,
  settingsDepartmentAccessQueryKey
} from '../../api/identity-access/departments';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { MemberManagementPanel } from '../../components/MemberManagementPanel';
import { i18nText } from '../../../../shared/i18n/text';

const DepartmentManagementPanel = lazy(() =>
  import('../../components/organization/DepartmentManagementPanel').then(
    ({ DepartmentManagementPanel }) => ({ default: DepartmentManagementPanel })
  )
);
const CreditManagementPanel = lazy(() =>
  import('../../components/billing/CreditManagementPanel').then(
    ({ CreditManagementPanel }) => ({ default: CreditManagementPanel })
  )
);

export function MemberSettingsTabs({
  canManageMembers
}: {
  canManageMembers: boolean;
}) {
  const navigate = useNavigate();
  const requestedTab = useRouterState({
    select: (state) => state.location.search.tabs
  });
  const activeTab =
    requestedTab === 'credits' || requestedTab === 'departments'
      ? requestedTab
      : 'members';
  const accessQuery = useQuery({
    queryKey: settingsDepartmentAccessQueryKey,
    queryFn: fetchSettingsDepartmentAccess,
    enabled: activeTab !== 'credits'
  });
  const departmentAccess = accessQuery.data ?? {
    can_list: false,
    can_create: false,
    can_update: false,
    can_delete: false,
    can_assign_roles: false,
    can_replace_member_departments: false
  };

  return (
    <Tabs
      activeKey={activeTab}
      onChange={(tabs) =>
        void navigate({ to: '/settings/members', search: { tabs } })
      }
      items={[
        {
          key: 'members',
          label: i18nText('settings', 'auto.user_management'),
          children: activeTab === 'members' ? (
            <MemberManagementPanel
              canManageMembers={canManageMembers}
              canManageRoleBindings={departmentAccess.can_assign_roles}
              canViewDepartments={departmentAccess.can_list}
              canCreateDepartments={departmentAccess.can_create}
              canManageMemberDepartments={
                departmentAccess.can_replace_member_departments
              }
            />
          ) : null
        },
        {
          key: 'departments',
          label: i18nText('settings', 'organization.management'),
          children: activeTab !== 'departments' ? null : accessQuery.isLoading ? (
            <Spin />
          ) : accessQuery.isError ? (
            <Alert
              type="error"
              title={i18nText('settings', 'organization.load_error')}
              action={
                <Button onClick={() => void accessQuery.refetch()}>
                  {i18nText('settings', 'auto.retry_permission_data')}
                </Button>
              }
            />
          ) : (
            <Suspense fallback={<Spin />}>
              <DepartmentManagementPanel access={departmentAccess} />
            </Suspense>
          )
        },
        {
          key: 'credits',
          label: i18nText('settings', 'auto.billing_user_credit'),
          children: activeTab === 'credits' ? (
            <Suspense fallback={<Spin />}>
              <CreditManagementPanel canManage={canManageMembers} />
            </Suspense>
          ) : null
        }
      ]}
    />
  );
}
