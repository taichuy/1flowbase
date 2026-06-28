import { useQuery } from '@tanstack/react-query';
import { Alert, Descriptions, Flex, Table, Tag } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import { i18nText } from '../../../../shared/i18n/text';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import {
  fetchSettingsAuthCenterOverview,
  settingsAuthCenterOverviewQueryKey,
  type SettingsAuthCenterOverview
} from '../../api/auth-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

type AuthenticatorRow =
  SettingsAuthCenterOverview['authenticators'][number];

const authenticatorColumns: ColumnsType<AuthenticatorRow> = [
  {
    title: i18nText('settings', 'auto.name'),
    dataIndex: 'name',
    key: 'name'
  },
  {
    title: i18nText('settings', 'auto.kind'),
    dataIndex: 'auth_type',
    key: 'auth_type'
  },
  {
    title: i18nText('settings', 'auto.title'),
    dataIndex: 'title',
    key: 'title'
  },
  {
    title: i18nText('settings', 'auto.status'),
    dataIndex: 'enabled',
    key: 'enabled',
    render: (enabled: boolean) => (
      <Tag color={enabled ? 'green' : 'default'}>
        {i18nText(
          'settings',
          enabled ? 'auto.enabled_alt' : 'auto.disabled'
        )}
      </Tag>
    )
  },
  {
    title: i18nText('settings', 'auto.built_in'),
    dataIndex: 'is_builtin',
    key: 'is_builtin',
    render: (isBuiltin: boolean) =>
      isBuiltin
        ? i18nText('settings', 'auto.yes')
        : i18nText('settings', 'auto.no')
  }
];

export function SettingsAuthCenterSection() {
  const overviewQuery = useQuery({
    queryKey: settingsAuthCenterOverviewQueryKey,
    queryFn: fetchSettingsAuthCenterOverview
  });

  return (
    <SettingsSectionSurface
      title={i18nText('settings', 'auto.auth_center')}
      hideHeader={false}
      heightMode="fill"
    >
      {overviewQuery.isLoading ? <LoadingState compact /> : null}
      {overviewQuery.isError ? (
        <Alert
          type="error"
          message={i18nText(
            'settings',
            'auto.auth_center_overview_load_failed'
          )}
        />
      ) : null}
      {overviewQuery.data ? (
        <Flex vertical gap="large">
          <Descriptions
            bordered
            size="small"
            column={{ xs: 1, sm: 2, lg: 3 }}
            items={[
              {
                key: 'default-authenticator',
                label: i18nText('settings', 'auto.default_authenticator'),
                children: overviewQuery.data.default_authenticator_name
              },
              {
                key: 'authenticators',
                label: i18nText('settings', 'auto.authenticators'),
                children: overviewQuery.data.authenticators.length
              },
              {
                key: 'sensitive-options',
                label: i18nText('settings', 'auto.sensitive_options'),
                children: i18nText('settings', 'auto.not_returned')
              }
            ]}
          />
          <Table
            rowKey="name"
            columns={authenticatorColumns}
            dataSource={overviewQuery.data.authenticators}
            pagination={false}
            size="middle"
          />
        </Flex>
      ) : null}
    </SettingsSectionSurface>
  );
}
