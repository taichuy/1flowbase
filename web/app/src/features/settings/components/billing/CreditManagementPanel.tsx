import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Form,
  Input,
  Modal,
  Space,
  Switch,
  Table,
  Tag,
  Typography
} from 'antd';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { fetchSettingsMembers } from '../../api/members';
import {
  executeSettingsCreditCommand,
  listSettingsCreditAccounts,
  listSettingsCreditLedger,
  settingsCreditAccountsQueryKey,
  settingsCreditLedgerQueryKey
} from '../../api/billing';
import { settingsMembersQueryKey } from '../../api/members';
import { SettingsSectionSurface } from '../SettingsSectionSurface';

type MoneyCommand = 'grant' | 'charge' | 'adjust' | 'refund';

const formatUsd = (value: string | undefined) => {
  const [integer = '0', fraction = ''] = (value ?? '0').split('.');
  const significantFraction = fraction.replace(/0+$/, '');
  const displayedFraction =
    significantFraction.length === 0
      ? '00'
      : significantFraction.length === 1
        ? `${significantFraction}0`
        : significantFraction;
  return `$${integer}.${displayedFraction}`;
};

function moneyCommandLabel(command: MoneyCommand) {
  switch (command) {
    case 'grant':
      return i18nText('settings', 'auto.billing_grant');
    case 'charge':
      return i18nText('settings', 'auto.billing_charge');
    case 'adjust':
      return i18nText('settings', 'auto.billing_adjust');
    case 'refund':
      return i18nText('settings', 'auto.billing_refund');
  }
}

export function CreditManagementPanel({ canManage }: { canManage: boolean }) {
  const csrfToken = useAuthStore((s) => s.csrfToken);
  const queryClient = useQueryClient();
  const [form] = Form.useForm();
  const [target, setTarget] = useState<{
    userId: string;
    command: MoneyCommand;
  } | null>(null);
  const [ledgerUser, setLedgerUser] = useState<string | null>(null);
  const members = useQuery({
    queryKey: settingsMembersQueryKey,
    queryFn: fetchSettingsMembers
  });
  const accounts = useQuery({
    queryKey: settingsCreditAccountsQueryKey,
    queryFn: () => listSettingsCreditAccounts()
  });
  const ledger = useQuery({
    queryKey: settingsCreditLedgerQueryKey(ledgerUser ?? undefined),
    queryFn: () => listSettingsCreditLedger(ledgerUser ?? undefined),
    enabled: ledgerUser !== null
  });
  const accountByUser = useMemo(
    () => new Map((accounts.data ?? []).map((a) => [a.user_id, a])),
    [accounts.data]
  );
  const mutate = useMutation({
    mutationFn: ({
      userId,
      command,
      amount,
      reason
    }: {
      userId: string;
      command: 'grant' | 'charge' | 'adjust' | 'refund' | 'enable' | 'disable';
      amount?: string;
      reason: string;
    }) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return executeSettingsCreditCommand(
        userId,
        command,
        {
          amount,
          reason,
          idempotency_key: `console:${command}:${userId}:${crypto.randomUUID()}`
        },
        csrfToken
      );
    },
    onSuccess: async () => {
      setTarget(null);
      form.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsCreditAccountsQueryKey
      });
    }
  });
  const rows = (members.data ?? []).map((member) => ({
    member,
    account: accountByUser.get(member.id)
  }));
  return (
    <SettingsSectionSurface heightMode="fill">
      <Table
        rowKey={(row) => row.member.id}
        loading={members.isLoading || accounts.isLoading}
        dataSource={rows}
        columns={[
          {
            title: i18nText('settings', 'auto.account_number'),
            render: (_, row) => (
              <>
                <Typography.Text strong>{row.member.account}</Typography.Text>
                <br />
                <Typography.Text type="secondary">
                  {row.member.name}
                </Typography.Text>
              </>
            )
          },
          {
            title: i18nText('settings', 'auto.billing_charge_enabled'),
            render: (_, row) => (
              <Switch
                checked={row.account?.charge_enabled ?? false}
                disabled={!canManage}
                loading={mutate.isPending}
                onChange={(checked) =>
                  mutate.mutate({
                    userId: row.member.id,
                    command: checked ? 'enable' : 'disable',
                    reason: 'console_toggle'
                  })
                }
              />
            )
          },
          {
            title: i18nText('settings', 'auto.billing_current_balance'),
            render: (_, row) => formatUsd(row.account?.current_balance)
          },
          {
            title: i18nText('settings', 'auto.billing_reserved_amount'),
            render: (_, row) => formatUsd(row.account?.reserved_amount)
          },
          {
            title: i18nText('settings', 'auto.billing_available_balance'),
            render: (_, row) => (
              <Tag color={row.account?.credit_insufficient ? 'red' : 'green'}>
                {formatUsd(row.account?.available_balance)}
              </Tag>
            )
          },
          {
            title: i18nText('settings', 'auto.operation'),
            render: (_, row) => (
              <Space wrap>
                {(
                  ['grant', 'charge', 'adjust', 'refund'] as MoneyCommand[]
                ).map((command) => (
                  <Button
                    key={command}
                    size="small"
                    disabled={!canManage}
                    onClick={() => {
                      setTarget({ userId: row.member.id, command });
                      form.setFieldsValue({ reason: `console_${command}` });
                    }}
                  >
                    {moneyCommandLabel(command)}
                  </Button>
                ))}
                <Button
                  size="small"
                  onClick={() => setLedgerUser(row.member.id)}
                >
                  {i18nText('settings', 'auto.billing_view_ledger')}
                </Button>
              </Space>
            )
          }
        ]}
        pagination={{ pageSize: 20 }}
        scroll={{ x: 1000 }}
      />
      <Modal
        open={target !== null}
        title={
          target ? moneyCommandLabel(target.command) : ''
        }
        onCancel={() => setTarget(null)}
        onOk={() => form.submit()}
        confirmLoading={mutate.isPending}
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={(v) =>
            target &&
            mutate.mutate({
              userId: target.userId,
              command: target.command,
              amount: String(v.amount),
              reason: String(v.reason)
            })
          }
        >
          <Form.Item
            name="amount"
            label={i18nText('settings', 'auto.billing_amount')}
            rules={[{ required: true }]}
          >
            <Input prefix="$" />
          </Form.Item>
          <Form.Item
            name="reason"
            label={i18nText('settings', 'auto.billing_reason')}
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
        </Form>
      </Modal>
      <Modal
        open={ledgerUser !== null}
        title={i18nText('settings', 'auto.billing_credit_ledger')}
        footer={null}
        width={900}
        onCancel={() => setLedgerUser(null)}
      >
        <Table
          rowKey="id"
          loading={ledger.isLoading}
          dataSource={ledger.data ?? []}
          columns={[
            {
              title: i18nText('settings', 'auto.billing_transaction_type'),
              dataIndex: 'transaction_type'
            },
            {
              title: i18nText('settings', 'auto.billing_amount'),
              render: (_, r) => formatUsd(r.amount)
            },
            {
              title: i18nText('settings', 'auto.billing_balance_after'),
              render: (_, r) => formatUsd(r.balance_after)
            },
            {
              title: i18nText('settings', 'auto.billing_reason'),
              dataIndex: 'reason'
            },
            {
              title: i18nText('settings', 'auto.billing_created_at'),
              dataIndex: 'created_at'
            }
          ]}
          pagination={{ pageSize: 10 }}
        />
      </Modal>
    </SettingsSectionSurface>
  );
}
