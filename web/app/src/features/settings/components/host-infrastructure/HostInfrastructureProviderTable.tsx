import { useMemo, useState } from 'react';

import { Button, Space, Table, Tag, Typography, App } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import type { SettingsHostInfrastructureProviderConfig } from '../../api/host-infrastructure';
import { HostInfrastructureProviderDrawer } from './HostInfrastructureProviderDrawer';
import { i18nText } from '../../../../shared/i18n/text';

export function HostInfrastructureProviderTable({
  providers,
  loading,
  canManage
}: {
  providers: SettingsHostInfrastructureProviderConfig[];
  loading: boolean;
  canManage: boolean;
}) {
  const [selectedProvider, setSelectedProvider] =
    useState<SettingsHostInfrastructureProviderConfig | null>(null);
  const { message: messageApi } = App.useApp();
  const columns = useMemo<
    ColumnsType<SettingsHostInfrastructureProviderConfig>
  >(
    () => [
      {
        title: 'Provider',
        dataIndex: 'display_name',
        key: 'provider',
        render: (_, provider) => (
          <Space orientation="vertical" size={2}>
            <Typography.Text strong>{provider.display_name}</Typography.Text>
            <Typography.Text type="secondary">
              {provider.extension_id} / {provider.provider_code}
            </Typography.Text>
          </Space>
        )
      },
      {
        title: 'Contracts',
        dataIndex: 'contracts',
        key: 'contracts',
        render: (contracts: string[]) => (
          <Space size={[4, 4]} wrap>
            {contracts.map((contract) => (
              <Tag key={contract}>{contract}</Tag>
            ))}
          </Space>
        )
      },
      {
        title: 'Desired',
        dataIndex: 'desired_state',
        key: 'desired_state',
        render: (state: string) => <Tag>{state}</Tag>
      },
      {
        title: 'Runtime',
        dataIndex: 'runtime_status',
        key: 'runtime_status',
        render: (status: string) => <Tag>{status}</Tag>
      },
      {
        title: 'Restart',
        dataIndex: 'restart_required',
        key: 'restart_required',
        render: (required: boolean) =>
          required ? (
            <Tag color="gold">
              {i18nText('settings', 'auto.take_effect_restart')}
            </Tag>
          ) : null
      },
      {
        title: '',
        key: 'actions',
        width: 96,
        render: (_, provider) => (
          <Button type="link" onClick={() => setSelectedProvider(provider)}>
            {i18nText('settings', 'auto.configuration_alt')}
          </Button>
        )
      }
    ],
    []
  );

  return (
    <>
      <Table
        rowKey={(provider) =>
          `${provider.installation_id}:${provider.provider_code}:${provider.config_ref}`
        }
        columns={columns}
        dataSource={providers}
        loading={loading}
        pagination={false}
      />
      {selectedProvider ? (
        <HostInfrastructureProviderDrawer
          provider={selectedProvider}
          canManage={canManage}
          open
          onSaved={() => {
            messageApi.success(
              i18nText(
                'settings',
                'auto.saved_take_effect_restarting_api_server'
              )
            );
          }}
          onClose={() => setSelectedProvider(null)}
        />
      ) : null}
    </>
  );
}
