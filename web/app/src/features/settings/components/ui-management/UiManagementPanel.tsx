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
  Modal,
  Select,
  Space,
  Table,
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
  type SettingsUiComponentCandidate,
  type SettingsUiComponentContract
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { CodeTemplatesTab } from './CodeTemplatesTab';

type ComponentFilter = {
  keyword: string;
  state?: SettingsUiComponentCandidate['state'];
};

type ContractDraftEditor =
  | { kind: 'prop'; index?: number }
  | { kind: 'limitation'; index?: number }
  | { kind: 'example'; index?: number };

const COMPONENT_PAGE_SIZE = 20;

function componentContractFormValue(
  candidate: SettingsUiComponentCandidate
): SettingsUiComponentContract {
  const contract =
    candidate.latest_contract ??
    candidate.published_contract ??
    candidate.official_contract;
  return {
    component_code: contract?.component_code ?? candidate.export_name,
    export_name: candidate.export_name,
    upstream: contract?.upstream ?? null,
    description: contract?.description ?? '',
    props: contract?.props ?? [],
    limitations: contract?.limitations ?? [''],
    examples: contract?.examples ?? [{ title: '', code: '' }],
    insert_snippet: contract?.insert_snippet ?? ''
  };
}

function requireToken(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

function ComponentsTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message } = App.useApp();
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
  const [form] = Form.useForm<SettingsUiComponentContract>();
  const [draftEntryForm] = Form.useForm();
  const [draftEditor, setDraftEditor] = useState<ContractDraftEditor | null>(
    null
  );
  const props = Form.useWatch('props', { form, preserve: true }) ?? [];
  const limitations = Form.useWatch('limitations', { form, preserve: true }) ?? [];
  const examples = Form.useWatch('examples', { form, preserve: true }) ?? [];
  const save = useMutation({
    mutationFn: async (contract: SettingsUiComponentContract) => {
      if (!selected) throw new Error('missing component');
      const upstream =
        contract.upstream?.package.trim() &&
        contract.upstream.component.trim() &&
        contract.upstream.version.trim()
          ? contract.upstream
          : null;
      return updateSettingsUiComponentContract(
        selected,
        { ...contract, export_name: selected.export_name, upstream },
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
      setDraftEditor(null);
      form.setFieldsValue(componentContractFormValue(row));
    },
    [form]
  );
  const openDraftEditor = (editor: ContractDraftEditor) => {
    setDraftEditor(editor);
    if (editor.kind === 'prop') {
      draftEntryForm.setFieldsValue(
        editor.index === undefined
          ? { name: '', type: '', description: '', required: false }
          : props[editor.index]
      );
      return;
    }
    if (editor.kind === 'limitation') {
      draftEntryForm.setFieldsValue({
        limitation: editor.index === undefined ? '' : limitations[editor.index]
      });
      return;
    }
    draftEntryForm.setFieldsValue(
      editor.index === undefined ? { title: '', code: '' } : examples[editor.index]
    );
  };
  const removeDraftEntry = (editor: Required<ContractDraftEditor>) => {
    if (editor.kind === 'prop') {
      form.setFieldValue(
        'props',
        props.filter((_, index) => index !== editor.index)
      );
      return;
    }
    if (editor.kind === 'limitation') {
      form.setFieldValue(
        'limitations',
        limitations.filter((_, index) => index !== editor.index)
      );
      return;
    }
    form.setFieldValue(
      'examples',
      examples.filter((_, index) => index !== editor.index)
    );
  };
  const saveDraftEntry = (values: Record<string, unknown>) => {
    if (!draftEditor) return;
    if (draftEditor.kind === 'prop') {
      const next = [...props];
      const entry = {
        name: String(values.name ?? ''),
        type: String(values.type ?? ''),
        description: String(values.description ?? ''),
        required: values.required === true
      };
      if (draftEditor.index === undefined) next.push(entry);
      else next[draftEditor.index] = entry;
      form.setFieldValue('props', next);
    } else if (draftEditor.kind === 'limitation') {
      const next = [...limitations];
      const entry = String(values.limitation ?? '');
      if (draftEditor.index === undefined) next.push(entry);
      else next[draftEditor.index] = entry;
      form.setFieldValue('limitations', next);
    } else {
      const next = [...examples];
      const entry = {
        title: String(values.title ?? ''),
        code: String(values.code ?? '')
      };
      if (draftEditor.index === undefined) next.push(entry);
      else next[draftEditor.index] = entry;
      form.setFieldValue('examples', next);
    }
    setDraftEditor(null);
  };
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
              {t('restore_official')}
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
          onFinish={(value) => save.mutate(value)}
        >
          <Form.Item
            name="component_code"
            label={t('component_code')}
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
          <Form.Item label={t('export_name')}>
            <Input
              aria-label={t('export_name')}
              disabled
              value={selected?.export_name}
            />
          </Form.Item>
          <Form.Item
            name="description"
            label={t('description')}
            rules={[{ required: true }]}
          >
            <Input.TextArea rows={3} />
          </Form.Item>
          <Form.Item
            name="insert_snippet"
            label={t('insert_snippet')}
            rules={[{ required: true }]}
          >
            <Input.TextArea rows={3} />
          </Form.Item>
          <Flex justify="space-between" align="center">
            <Typography.Title level={5}>{t('props')}</Typography.Title>
            <Button onClick={() => openDraftEditor({ kind: 'prop' })}>
              {t('new_prop')}
            </Button>
          </Flex>
          <Table
            columns={[
              { title: t('prop_name'), dataIndex: 'name' },
              { title: t('prop_type'), dataIndex: 'type' },
              { title: t('description'), dataIndex: 'description' },
              {
                title: t('required'),
                dataIndex: 'required',
                render: (required) => t(required ? 'required' : 'optional')
              },
              {
                title: t('actions'),
                render: (_, __, index) => (
                  <Space>
                    <Button type="link" onClick={() => openDraftEditor({ kind: 'prop', index })}>
                      {t('edit')}
                    </Button>
                    <Button type="link" danger onClick={() => removeDraftEntry({ kind: 'prop', index })}>
                      {t('remove')}
                    </Button>
                  </Space>
                )
              }
            ]}
            dataSource={props.map((prop, index) => ({ ...prop, key: index }))}
            pagination={false}
            rowKey="key"
            size="small"
          />
          <Flex justify="space-between" align="center">
            <Typography.Title level={5}>{t('limitations')}</Typography.Title>
            <Button onClick={() => openDraftEditor({ kind: 'limitation' })}>
              {t('new_limitation')}
            </Button>
          </Flex>
          <Table
            columns={[
              { title: t('limitations'), dataIndex: 'limitation' },
              {
                title: t('actions'),
                render: (_, __, index) => (
                  <Space>
                    <Button type="link" onClick={() => openDraftEditor({ kind: 'limitation', index })}>
                      {t('edit')}
                    </Button>
                    <Button type="link" danger onClick={() => removeDraftEntry({ kind: 'limitation', index })}>
                      {t('remove')}
                    </Button>
                  </Space>
                )
              }
            ]}
            dataSource={limitations.map((limitation, index) => ({ limitation, key: index }))}
            pagination={false}
            rowKey="key"
            size="small"
          />
          <Flex justify="space-between" align="center">
            <Typography.Title level={5}>{t('examples')}</Typography.Title>
            <Button onClick={() => openDraftEditor({ kind: 'example' })}>
              {t('new_example')}
            </Button>
          </Flex>
          <Table
            columns={[
              { title: t('example_title'), dataIndex: 'title' },
              { title: t('example_code'), dataIndex: 'code' },
              {
                title: t('actions'),
                render: (_, __, index) => (
                  <Space>
                    <Button type="link" onClick={() => openDraftEditor({ kind: 'example', index })}>
                      {t('edit')}
                    </Button>
                    <Button type="link" danger onClick={() => removeDraftEntry({ kind: 'example', index })}>
                      {t('remove')}
                    </Button>
                  </Space>
                )
              }
            ]}
            dataSource={examples.map((example, index) => ({ ...example, key: index }))}
            pagination={false}
            rowKey="key"
            size="small"
          />
          <Typography.Title level={5}>{t('upstream')}</Typography.Title>
          <Flex vertical gap={8}>
            <Form.Item name={['upstream', 'package']} style={{ width: '100%' }}>
              <Input placeholder={t('upstream_package')} />
            </Form.Item>
            <Form.Item name={['upstream', 'component']} style={{ width: '100%' }}>
              <Input placeholder={t('upstream_component')} />
            </Form.Item>
            <Form.Item name={['upstream', 'version']} style={{ width: '100%' }}>
              <Input placeholder={t('upstream_version')} />
            </Form.Item>
          </Flex>
        </Form>
        <Modal
          open={draftEditor !== null}
          title={
            draftEditor?.kind === 'prop'
              ? t(draftEditor.index === undefined ? 'new_prop' : 'edit_prop')
              : draftEditor?.kind === 'limitation'
                ? t(
                    draftEditor.index === undefined
                      ? 'new_limitation'
                      : 'edit_limitation'
                  )
                : t(draftEditor?.index === undefined ? 'new_example' : 'edit_example')
          }
          okText={t('save')}
          onCancel={() => setDraftEditor(null)}
          onOk={() => draftEntryForm.submit()}
        >
          <Form
            form={draftEntryForm}
            layout="vertical"
            name="component-contract-draft"
            onFinish={saveDraftEntry}
          >
            {draftEditor?.kind === 'prop' ? (
              <>
                <Form.Item name="name" label={t('prop_name')} rules={[{ required: true }]}>
                  <Input />
                </Form.Item>
                <Form.Item name="type" label={t('prop_type')} rules={[{ required: true }]}>
                  <Input />
                </Form.Item>
                <Form.Item name="description" label={t('description')} rules={[{ required: true }]}>
                  <Input.TextArea rows={3} />
                </Form.Item>
                <Form.Item name="required" label={t('required')}>
                  <Select
                    options={[
                      { value: true, label: t('required') },
                      { value: false, label: t('optional') }
                    ]}
                  />
                </Form.Item>
              </>
            ) : draftEditor?.kind === 'limitation' ? (
              <Form.Item name="limitation" label={t('limitations')} rules={[{ required: true }]}>
                <Input.TextArea rows={3} />
              </Form.Item>
            ) : (
              <>
                <Form.Item name="title" label={t('example_title')} rules={[{ required: true }]}>
                  <Input />
                </Form.Item>
                <Form.Item name="code" label={t('example_code')} rules={[{ required: true }]}>
                  <Input.TextArea rows={4} />
                </Form.Item>
              </>
            )}
          </Form>
        </Modal>
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
