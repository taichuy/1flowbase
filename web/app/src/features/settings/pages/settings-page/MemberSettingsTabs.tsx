import { Tabs } from 'antd';
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
  return (
    <Tabs
      defaultActiveKey="members"
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
