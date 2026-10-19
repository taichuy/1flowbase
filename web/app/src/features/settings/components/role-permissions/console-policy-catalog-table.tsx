import { Button, Select, Space, Switch, Table, Typography } from 'antd';
import type { SettingsConsolePolicyCatalog } from '../../api/permissions';
import type { SettingsRole } from '../../api/roles';
import { i18nText } from '../../../../shared/i18n/text';
import { PolicyDragHandle, SortablePolicyRow, SortablePolicyTable } from './SortablePolicyTable';
import { replaceConsolePolicyGroup, type ConsolePolicyCatalogGroup, type ConsolePolicyGroup } from './console-policy-draft';

type ConsolePolicyCatalogTableContext = {
  catalogGroups: SettingsConsolePolicyCatalog['groups'];
  groupStrategyOptions: SettingsConsolePolicyCatalog['group_strategy_options'];
  policyGroupForCatalogGroup: (group: ConsolePolicyCatalogGroup) => ConsolePolicyGroup;
  canManageRoles: boolean;
  selectedRole: SettingsRole | null | undefined;
  replaceConsolePolicyMutation: { isPending: boolean };
  consolePolicyGroups: ConsolePolicyGroup[];
  saveConsolePolicyGroups: (groups: ConsolePolicyGroup[]) => void;
  openConsolePolicyDetail: (group: ConsolePolicyCatalogGroup, forceCustom?: boolean) => void;
  consoleSettingsOrder: { reorder: (oldIndex: number, newIndex: number) => void };
};

export function renderConsolePolicyCatalogTable(
  kind: ConsolePolicyCatalogGroup['kind'],
  {
    catalogGroups, groupStrategyOptions, policyGroupForCatalogGroup,
    canManageRoles, selectedRole, replaceConsolePolicyMutation,
    consolePolicyGroups, saveConsolePolicyGroups, openConsolePolicyDetail,
    consoleSettingsOrder
  }: ConsolePolicyCatalogTableContext
) {
  const policyTableRows = (kind: ConsolePolicyCatalogGroup['kind']) =>
    catalogGroups
      .filter((catalogGroup) => catalogGroup.kind === kind)
      .map((catalogGroup, displayKey) => ({
        display_key: displayKey,
        catalogGroup,
        policyGroup: policyGroupForCatalogGroup(catalogGroup)
      }));


  const rows = policyTableRows(kind);
  const table = (
    <Table
      rowKey="display_key"
      pagination={false}
      scroll={{ x: 'max-content' }}
      dataSource={rows}
      components={
        kind === 'settings_feature'
          ? { body: { row: SortablePolicyRow } }
          : undefined
      }
      columns={[
        {
          title: i18nText('settings', 'auto.backend_setting'),
          key: 'backend-setting',
          render: (
            _: unknown,
            row: ReturnType<typeof policyTableRows>[number]
          ) => (
            <Space align="start">
              {kind === 'settings_feature' ? <PolicyDragHandle /> : null}
              <Space orientation="vertical" size={0}>
                <Typography.Text strong>
                  {row.catalogGroup.label}
                </Typography.Text>
                {row.catalogGroup.description ? (
                  <Typography.Text type="secondary">
                    {row.catalogGroup.description}
                  </Typography.Text>
                ) : null}
              </Space>
            </Space>
          )
        },
        {
          title: i18nText('settings', 'auto.enabled'),
          key: 'authorization-enabled',
          width: 100,
          render: (
            _: unknown,
            row: ReturnType<typeof policyTableRows>[number]
          ) => (
            <Switch
              aria-label={`${row.catalogGroup.label} ${i18nText('settings', 'auto.enabled')}`}
              checked={row.policyGroup.enabled}
              disabled={
                !canManageRoles ||
                !selectedRole?.is_editable ||
                replaceConsolePolicyMutation.isPending
              }
              onChange={(enabled) =>
                saveConsolePolicyGroups(
                  replaceConsolePolicyGroup(consolePolicyGroups, {
                    ...row.policyGroup,
                    enabled
                  })
                )
              }
            />
          )
        },
        {
          title: i18nText('settings', 'auto.authorization_policy'),
          key: 'authorization-policy',
          width: 180,
          render: (
            _: unknown,
            row: ReturnType<typeof policyTableRows>[number]
          ) => (
            <Select
              aria-label={`${row.catalogGroup.label} ${i18nText(
                'settings',
                'auto.authorization_policy'
              )}`}
              value={row.policyGroup.strategy}
              disabled={
                !canManageRoles ||
                !selectedRole?.is_editable ||
                replaceConsolePolicyMutation.isPending
              }
              options={groupStrategyOptions.map((option) => ({
                value: option.value,
                label: option.label,
                title: option.label
              }))}
              style={{ width: 160 }}
              onChange={(strategy: ConsolePolicyGroup['strategy']) => {
                if (strategy === 'custom') {
                  openConsolePolicyDetail(row.catalogGroup, true);
                  return;
                }

                saveConsolePolicyGroups(
                  replaceConsolePolicyGroup(consolePolicyGroups, {
                    ...row.policyGroup,
                    strategy
                  })
                );
              }}
            />
          )
        },
        {
          title: i18nText('settings', 'auto.operation'),
          key: 'policy-detail',
          render: (
            _: unknown,
            row: ReturnType<typeof policyTableRows>[number]
          ) => {
            const canEdit = canManageRoles && selectedRole?.is_editable;
            return (
              <Button
                type="link"
                aria-label={i18nText(
                  'settings',
                  'auto.permission_policy_details',
                  { value1: row.catalogGroup.label }
                )}
                disabled={!canEdit}
                onClick={() => openConsolePolicyDetail(row.catalogGroup)}
              >
                {i18nText(
                  'settings',
                  'auto.permission_policy_details_text'
                )}
              </Button>
            );
          }
        }
      ]}
    />
  );
  if (kind !== 'settings_feature') return table;
  const ids = rows.map((row) => String(row.display_key));
  return (
    <SortablePolicyTable
      itemIds={ids}
      onReorder={consoleSettingsOrder.reorder}
    >
      {table}
    </SortablePolicyTable>
  );
}
