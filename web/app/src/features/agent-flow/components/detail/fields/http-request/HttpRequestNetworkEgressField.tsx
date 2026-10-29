import type { HttpNodeNetworkEgress } from '@1flowbase/api-client';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Select, Space, Typography } from 'antd';

import type { SchemaFieldRendererProps } from '../../../../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { i18nText } from '../../../../../../shared/i18n/text';
import {
  fetchHttpNodeProxyOptions,
  httpNodeProxyOptionsQueryKey
} from '../../../../api/http-node-proxy-options';

export function HttpRequestNetworkEgressField({
  adapter,
  block
}: SchemaFieldRendererProps) {
  const value = (adapter.getValue(block.path) ?? {
    mode: 'workspace'
  }) as HttpNodeNetworkEgress;
  const optionsQuery = useQuery({
    queryKey: httpNodeProxyOptionsQueryKey,
    queryFn: fetchHttpNodeProxyOptions,
    enabled: value.mode === 'proxy',
    staleTime: 0
  });
  const options = (optionsQuery.data ?? []).map((option) => ({
    value: option.pool_member_id,
    label: option.selectable
      ? option.display_name
      : `${option.display_name} (${i18nText('agentFlow', 'network_egress.unavailable')})`,
    disabled: !option.selectable
  }));
  const selectedId = value.mode === 'proxy' ? value.pool_member_id : undefined;
  if (selectedId && !options.some((option) => option.value === selectedId)) {
    options.push({
      value: selectedId,
      label: i18nText('agentFlow', 'network_egress.unavailable'),
      disabled: true
    });
  }

  return (
    <Space orientation="vertical" style={{ width: '100%' }}>
      <Select
        aria-label={block.label}
        style={{ width: '100%' }}
        value={value.mode}
        options={[
          {
            value: 'workspace',
            label: i18nText('agentFlow', 'network_egress.workspace')
          },
          {
            value: 'proxy',
            label: i18nText('agentFlow', 'network_egress.proxy')
          },
          {
            value: 'direct',
            label: i18nText('agentFlow', 'network_egress.direct')
          }
        ]}
        onChange={(mode: HttpNodeNetworkEgress['mode']) =>
          adapter.setValue(
            block.path,
            mode === 'proxy' ? { mode, pool_member_id: '' } : { mode }
          )
        }
      />
      {value.mode === 'proxy' && (
        <>
          <Select
            aria-label={i18nText('agentFlow', 'network_egress.proxy')}
            placeholder={i18nText('agentFlow', 'network_egress.choose')}
            style={{ width: '100%' }}
            showSearch
            optionFilterProp="label"
            loading={optionsQuery.isFetching}
            disabled={optionsQuery.isError}
            value={selectedId || undefined}
            options={options}
            onChange={(pool_member_id: string) =>
              adapter.setValue(block.path, { mode: 'proxy', pool_member_id })
            }
          />
          {optionsQuery.isError && (
            <Alert
              type="error"
              title={i18nText('agentFlow', 'network_egress.load_failed')}
              action={
                <Button onClick={() => void optionsQuery.refetch()}>
                  {i18nText('agentFlow', 'network_egress.retry')}
                </Button>
              }
            />
          )}
          <Typography.Text type="secondary">
            {i18nText('agentFlow', 'network_egress.failure_policy')}
          </Typography.Text>
        </>
      )}
    </Space>
  );
}
