import { Tabs } from 'antd';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { CreditManagementPanel } from '../../components/billing/CreditManagementPanel';
import { MemberManagementPanel } from '../../components/MemberManagementPanel';
import { i18nText } from '../../../../shared/i18n/text';

export function MemberSettingsTabs({
  canManageMembers,
  canManageRoleBindings
}: {
  canManageMembers: boolean;
  canManageRoleBindings: boolean;
}) {
  const navigate = useNavigate();
  const activeTab = useRouterState({
    select: (state) => state.location.search.tabs
  });

  return (
    <Tabs
      activeKey={activeTab === 'credits' ? 'credits' : 'members'}
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
              canManageRoleBindings={canManageRoleBindings}
            />
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
