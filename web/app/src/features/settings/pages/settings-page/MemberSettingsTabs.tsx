import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Spin } from 'antd';
import {
  fetchSettingsDepartmentAccess,
  settingsDepartmentAccessQueryKey
} from '../../api/departments';
import { DepartmentManagementPanel } from '../../components/organization/DepartmentManagementPanel';
import { Tabs } from 'antd';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { CreditManagementPanel } from '../../components/billing/CreditManagementPanel';
import { MemberManagementPanel } from '../../components/MemberManagementPanel';
import { i18nText } from '../../../../shared/i18n/text';

export function MemberSettingsTabs({
  canManageMembers
}: {
  canManageMembers: boolean;
}) {
  const navigate = useNavigate();
  const accessQuery = useQuery({
    queryKey: settingsDepartmentAccessQueryKey,
    queryFn: fetchSettingsDepartmentAccess
  });
  const departmentAccess = accessQuery.data ?? {
    can_list: false,
    can_create: false,
    can_update: false,
    can_delete: false,
    can_assign_roles: false,
    can_replace_member_departments: false
  };
  const activeTab = useRouterState({
    select: (state) => state.location.search.tabs
  });

  return (
    <Tabs
      activeKey={
        activeTab === 'credits'
          ? 'credits'
          : activeTab === 'departments'
            ? 'departments'
            : 'members'
      }
      onChange={(tabs) =>
        void navigate({ to: '/settings/members', search: { tabs } })
      }
      items={[
        {
          key: 'members',
          label: i18nText('settings', 'auto.user_management'),
          children: (
            <MemberManagementPanel
              canManageMembers={canManageMembers}
              canManageRoleBindings={departmentAccess.can_assign_roles}
              canViewDepartments={departmentAccess.can_list}
              canCreateDepartments={departmentAccess.can_create}
              canManageMemberDepartments={
                departmentAccess.can_replace_member_departments
              }
            />
          )
        },
        {
          key: 'departments',
          label: i18nText('settings', 'organization.management'),
          children: accessQuery.isLoading ? (
            <Spin />
          ) : accessQuery.isError ? (
            <Alert
              type="error"
              message={i18nText('settings', 'organization.load_error')}
              action={
                <Button onClick={() => void accessQuery.refetch()}>
                  {i18nText('settings', 'auto.retry_permission_data')}
                </Button>
              }
            />
          ) : (
            <DepartmentManagementPanel access={departmentAccess} />
          )
        },
        {
          key: 'credits',
          label: i18nText('settings', 'auto.billing_user_credit'),
          children: <CreditManagementPanel canManage={canManageMembers} />
        }
      ]}
    />
  );
}
