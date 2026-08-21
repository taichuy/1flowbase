import { useCallback, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import {
  App,
  Button,
  Drawer,
  Flex,
  Form,
  Grid,
  Input,
  Select,
  Space,
  Tabs,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  fetchSettingsUiComponents,
  settingsUiComponentsQueryKey,
  updateSettingsUiComponentContract,
  updateSettingsUiComponentState,
  type SettingsUiComponentCandidate
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { CodeTemplatesTab } from './CodeTemplatesTab';

type ComponentFilter = {
  keyword: string;
  state?: SettingsUiComponentCandidate['state'];
};

const COMPONENT_PAGE_SIZE = 20;

function requireToken(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

function ComponentsTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message, modal } = App.useApp();
  const csrfToken = useAuthStore((s) => s.csrfToken);
  const client = useQueryClient();
  const query = useQuery({
    queryKey: settingsUiComponentsQueryKey,
    queryFn: () => fetchSettingsUiComponents()
  });
  const [selected, setSelected] = useState<SettingsUiComponentCandidate | null>(
    null
  );
  const [page, setPage] = useState(1);
  const [filterDraft, setFilterDraft] = useState<ComponentFilter>({
    keyword: ''
  });
  const [filter, setFilter] = useState<ComponentFilter>({ keyword: '' });
  const [form] = Form.useForm<{ contract: string }>();
  const save = useMutation({
    mutationFn: async (value: { contract: string }) => {
      if (!selected) throw new Error('missing component');
      const contract = JSON.parse(value.contract) as Record<string, unknown>;
      return updateSettingsUiComponentContract(
        selected,
        contract,
        requireToken(csrfToken)
      );
    },
    onSuccess: async () => {
      await client.invalidateQueries({
        queryKey: settingsUiComponentsQueryKey
      });
      setSelected(null);
      message.success(t('saved'));
    }
  });
  const state = useMutation({
    mutationFn: async (input: {
      row: SettingsUiComponentCandidate;
      state: 'inherit' | 'published' | 'hidden';
    }) =>
      updateSettingsUiComponentState(
        input.row,
        input.state,
        requireToken(csrfToken)
      ),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: settingsUiComponentsQueryKey })
  });
  const mutateState = state.mutate;
  const edit = useCallback(
    (row: SettingsUiComponentCandidate) => {
      setSelected(row);
      form.setFieldsValue({
        contract: JSON.stringify(
          row.latest_contract ??
            row.published_contract ??
            row.official_contract ?? {
              component_code: row.export_name,
              export_name: row.export_name,
              upstream: null,
              description: '',
              props: [],
              limitations: [''],
              examples: [{ title: '', code: '' }],
              insert_snippet: `<${row.export_name} />`
            },
          null,
          2
        )
      });
    },
    [form]
  );
  const columns = useMemo<Array<DataTableColumn<SettingsUiComponentCandidate>>>(
    () => [
      {
        key: 'component',
        title: t('component'),
        dataIndex: 'export_name',
        width: 180
      },
      {
        key: 'module_source',
        title: t('module_source'),
        width: 280,
        sizing: 'fill',
        dataIndex: 'module_source'
      },
      {
        key: 'module_version',
        title: t('module_version'),
        width: 120,
        dataIndex: 'module_version'
      },
      {
        key: 'contribution',
        title: t('contribution'),
        width: 220,
        render: (_, row) => `${row.provider_code}/${row.contribution_code}`
      },
      {
        key: 'status',
        title: t('status'),
        width: 120,
        render: (_, row) => (
          <Tag
            color={
              row.state === 'published'
                ? 'green'
                : row.state === 'hidden'
                  ? 'red'
                  : 'default'
            }
          >
            {t(row.state)}
          </Tag>
        )
      },
      {
        key: 'revision',
        title: t('revision'),
        width: 100,
        render: (_, row) =>
          row.latest_revision ? `r${row.latest_revision}` : '—'
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 420,
        minWidth: 420,
        align: 'center',
        render: (_, row) => (
          <Space wrap>
            <Button
              type="link"
              size="small"
              disabled={!canManage}
              onClick={() => edit(row)}
            >
              {t('edit')}
            </Button>
            <Button
              type="link"
              size="small"
              disabled={!canManage || !row.latest_contract}
              onClick={() => mutateState({ row, state: 'published' })}
            >
              {t('publish')}
            </Button>
            <Button
              type="link"
              size="small"
              danger
              disabled={!canManage || row.state === 'hidden'}
              onClick={() => mutateState({ row, state: 'hidden' })}
            >
              {t('hide')}
            </Button>
            <Button
              type="link"
              size="small"
              disabled={!canManage || row.state === 'inherit'}
              onClick={() => mutateState({ row, state: 'inherit' })}
            >
              {t('default')}
            </Button>
          </Space>
        )
      }
    ],
    [canManage, edit, mutateState, t]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.ui_management.components'
  });
  const filteredRows = useMemo(() => {
    const keyword = filter.keyword.trim().toLocaleLowerCase();
    return (query.data ?? []).filter((row) => {
      if (filter.state && row.state !== filter.state) return false;
      if (!keyword) return true;
      return [
        row.export_name,
        row.module_source,
        row.module_version,
        row.provider_code,
        row.contribution_code
      ].some((value) => value.toLocaleLowerCase().includes(keyword));
    });
  }, [filter, query.data]);
  const visibleRows = filteredRows.slice(
    (page - 1) * COMPONENT_PAGE_SIZE,
    page * COMPONENT_PAGE_SIZE
  );
  return (
    <SettingsSectionSurface heightMode="fill">
      <DataTableLayout
        filters={
          <DataTableFilterForm
            ariaLabel={t('component_filter_submit')}
            resetLabel={t('component_filter_reset')}
            submitLabel={t('component_filter_submit')}
            onReset={() => {
              const emptyFilter = { keyword: '' };
              setFilterDraft(emptyFilter);
              setFilter(emptyFilter);
              setPage(1);
            }}
            onSubmit={() => {
              setFilter(filterDraft);
              setPage(1);
            }}
          >
            <DataTableFilterField label={t('component_filter_keyword')}>
              <Input
                aria-label={t('component_filter_search')}
                placeholder={t('component_filter_search')}
                type="search"
                value={filterDraft.keyword}
                onChange={(event) =>
                  setFilterDraft((current) => ({
                    ...current,
                    keyword: event.target.value
                  }))
                }
              />
            </DataTableFilterField>
            <DataTableFilterField label={t('status')}>
              <Select<SettingsUiComponentCandidate['state']>
                allowClear
                aria-label={t('component_filter_status')}
                placeholder={t('component_filter_all_statuses')}
                value={filterDraft.state}
                options={(['inherit', 'published', 'hidden'] as const).map(
                  (value) => ({ value, label: t(value) })
                )}
                onChange={(value) =>
                  setFilterDraft((current) => ({ ...current, state: value }))
                }
              />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
        <DataTable<SettingsUiComponentCandidate>
          columns={columns}
          configuration={tableConfiguration}
          dataSource={visibleRows}
          loading={query.isLoading || query.isFetching}
          page={page}
          pageSize={COMPONENT_PAGE_SIZE}
          rowKey={(row) =>
            `${row.provider_code}:${row.contribution_code}:${row.module_source}:${row.export_name}`
          }
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <Button onClick={() => query.refetch()}>
                {t('component_refresh')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
          total={filteredRows.length}
          onPageChange={setPage}
        />
      </DataTableLayout>
      <Drawer
        open={!!selected}
        width={720}
        title={selected ? `${t('edit_contract')}: ${selected.export_name}` : ''}
        onClose={() => setSelected(null)}
        extra={
          <Button
            type="primary"
            loading={save.isPending}
            onClick={() => form.submit()}
          >
            {t('save_revision')}
          </Button>
        }
      >
        <Typography.Paragraph type="secondary">
          {t('contract_help')}
        </Typography.Paragraph>
        <Form
          form={form}
          layout="vertical"
          onFinish={(v) => {
            try {
              JSON.parse(v.contract);
              save.mutate(v);
            } catch {
              modal.error({ title: t('invalid_json') });
            }
          }}
        >
          <Form.Item
            name="contract"
            label={t('contract_json')}
            rules={[{ required: true }]}
          >
            <Input.TextArea
              autoSize={{ minRows: 22, maxRows: 36 }}
              className="ui-management-code-editor"
            />
          </Form.Item>
        </Form>
      </Drawer>
    </SettingsSectionSurface>
  );
}

export function UiManagementPanel({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const navigate = useNavigate();
  const screens = Grid.useBreakpoint();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const active = pathname.endsWith('/components')
    ? 'components'
    : 'code-templates';
  const fillViewport = screens.lg !== false;
  const fillStyle = fillViewport ? { height: '100%', minHeight: 0 } : undefined;
  return (
    <>
      <Tabs
        className="ui-management-panel"
        styles={{ root: fillStyle, body: fillStyle, content: fillStyle }}
        activeKey={active}
        onChange={(key) =>
          navigate({
            to:
              key === 'components'
                ? '/settings/ui-management/components'
                : '/settings/ui-management/code-templates'
          })
        }
        items={[
          {
            key: 'code-templates',
            label: t('code_templates'),
            style: fillStyle,
            children: <CodeTemplatesTab canManage={canManage} />
          },
          {
            key: 'components',
            label: t('components'),
            style: fillStyle,
            children: <ComponentsTab canManage={canManage} />
          }
        ]}
      />
    </>
  );
}
