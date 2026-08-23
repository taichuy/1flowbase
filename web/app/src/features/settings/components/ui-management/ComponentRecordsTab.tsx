import { useCallback, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  App,
  Button,
  Descriptions,
  Flex,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Tag,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';

import { BlockSourceEditor } from '../../../../shared/code-block/BlockSourceEditor';
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
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  createSettingsUiComponent,
  deleteSettingsUiComponent,
  fetchSettingsUiComponent,
  fetchSettingsUiComponents,
  settingsUiComponentsQueryKey,
  updateSettingsUiComponent,
  type CreateSettingsUiComponentInput,
  type SettingsUiComponentRecord,
  type UpdateSettingsUiComponentInput
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { RemoteCatalogDrawer } from './RemoteCatalogDrawer';

type DrawerMode = 'create' | 'edit' | 'detail';
type FormValue = Omit<CreateSettingsUiComponentInput, 'keywords'> & {
  keywords: string;
};

const PAGE_SIZE = 20;

function tokenOrThrow(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

function formValue(record?: SettingsUiComponentRecord): FormValue {
  return {
    component_code: record?.component_code ?? '',
    name: record?.name ?? '',
    description: record?.description ?? '',
    import_code: record?.import_code ?? '',
    source_code: record?.source_code ?? '',
    source: record?.source ?? '',
    group: record?.group ?? '',
    upstream: record?.upstream ?? { identity: '', version: '' },
    version: record?.version ?? '1.0.0',
    keywords: record?.keywords.join(',') ?? ''
  };
}

function requestValue(value: FormValue): CreateSettingsUiComponentInput {
  return {
    ...value,
    keywords: value.keywords
      .split(',')
      .map((keyword) => keyword.trim())
      .filter(Boolean)
  };
}

export function ComponentRecordsTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [form] = Form.useForm<FormValue>();
  const [page, setPage] = useState(1);
  const [keywordDraft, setKeywordDraft] = useState('');
  const [keyword, setKeyword] = useState('');
  const [origin, setOrigin] = useState<SettingsUiComponentRecord['origin']>();
  const [originDraft, setOriginDraft] =
    useState<SettingsUiComponentRecord['origin']>();
  const [drawerMode, setDrawerMode] = useState<DrawerMode>();
  const [selected, setSelected] = useState<SettingsUiComponentRecord>();
  const [deleteTarget, setDeleteTarget] = useState<SettingsUiComponentRecord>();
  const [catalogOpen, setCatalogOpen] = useState(false);
  const importCode =
    Form.useWatch('import_code', { form, preserve: true }) ?? '';
  const sourceCode =
    Form.useWatch('source_code', { form, preserve: true }) ?? '';
  const query = useQuery({
    queryKey: settingsUiComponentsQueryKey,
    queryFn: () => fetchSettingsUiComponents()
  });

  const openRecord = useCallback(
    async (record: SettingsUiComponentRecord, mode: 'edit' | 'detail') => {
      const detail = await fetchSettingsUiComponent(record.id);
      setSelected(detail);
      setDrawerMode(mode);
      form.setFieldsValue(formValue(detail));
    },
    [form]
  );
  const create = () => {
    setSelected(undefined);
    setDrawerMode('create');
    form.setFieldsValue(formValue());
  };
  const closeDrawer = () => {
    setDrawerMode(undefined);
    setSelected(undefined);
    form.resetFields();
  };

  const save = useMutation({
    mutationFn: async (value: FormValue) => {
      const request = requestValue(value);
      if (drawerMode === 'edit' && selected) {
        const patch: UpdateSettingsUiComponentInput = {
          name: request.name,
          description: request.description,
          import_code: request.import_code,
          source_code: request.source_code,
          source: request.source,
          group: request.group,
          upstream: request.upstream,
          version: request.version,
          keywords: request.keywords
        };
        return updateSettingsUiComponent(
          selected.id,
          patch,
          tokenOrThrow(csrfToken)
        );
      }
      return createSettingsUiComponent(request, tokenOrThrow(csrfToken));
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsUiComponentsQueryKey
      });
      closeDrawer();
      message.success(t('component_saved'));
    }
  });
  const remove = useMutation({
    mutationFn: (record: SettingsUiComponentRecord) =>
      deleteSettingsUiComponent(record.id, tokenOrThrow(csrfToken)),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsUiComponentsQueryKey
      });
      setDeleteTarget(undefined);
      message.success(t('component_deleted'));
    }
  });

  const columns = useMemo<Array<DataTableColumn<SettingsUiComponentRecord>>>(
    () => [
      { key: 'name', title: t('name'), dataIndex: 'name', width: 180 },
      {
        key: 'component_code',
        title: t('component_code'),
        dataIndex: 'component_code',
        width: 260,
        sizing: 'fill'
      },
      { key: 'source', title: t('source'), dataIndex: 'source', width: 120 },
      { key: 'group', title: t('group'), dataIndex: 'group', width: 160 },
      { key: 'version', title: t('version'), dataIndex: 'version', width: 100 },
      {
        key: 'origin',
        title: t('origin'),
        width: 110,
        render: (_, record) => <Tag>{t(record.origin)}</Tag>
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 240,
        align: 'center',
        render: (_, record) => (
          <Space>
            <Button
              type="link"
              size="small"
              aria-label={`${t('view')} ${record.name}`}
              onClick={() => void openRecord(record, 'detail')}
            >
              {t('view')}
            </Button>
            {record.origin === 'custom' && canManage ? (
              <>
                <Button
                  type="link"
                  size="small"
                  aria-label={`${t('edit')} ${record.name}`}
                  onClick={() => void openRecord(record, 'edit')}
                >
                  {t('edit')}
                </Button>
                <Button
                  type="link"
                  size="small"
                  danger
                  aria-label={`${t('delete')} ${record.name}`}
                  onClick={() => setDeleteTarget(record)}
                >
                  {t('delete')}
                </Button>
              </>
            ) : null}
          </Space>
        )
      }
    ],
    [canManage, openRecord, t]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.ui_management.components'
  });
  const filtered = useMemo(() => {
    const normalized = keyword.trim().toLocaleLowerCase();
    return (query.data ?? []).filter((record) => {
      if (origin && record.origin !== origin) return false;
      if (!normalized) return true;
      return [
        record.name,
        record.component_code,
        record.description,
        record.source,
        record.group,
        ...record.keywords
      ].some((value) => value.toLocaleLowerCase().includes(normalized));
    });
  }, [keyword, origin, query.data]);
  const visible = filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  const readOnly = drawerMode === 'detail';

  return (
    <SettingsSectionSurface heightMode="fill">
      <DataTableLayout
        filters={
          <DataTableFilterForm
            ariaLabel={t('component_filter_submit')}
            submitLabel={t('component_filter_submit')}
            resetLabel={t('component_filter_reset')}
            onSubmit={() => {
              setKeyword(keywordDraft);
              setOrigin(originDraft);
              setPage(1);
            }}
            onReset={() => {
              setKeywordDraft('');
              setKeyword('');
              setOriginDraft(undefined);
              setOrigin(undefined);
              setPage(1);
            }}
          >
            <DataTableFilterField label={t('component_filter_keyword')}>
              <Input
                type="search"
                aria-label={t('component_filter_search')}
                value={keywordDraft}
                onChange={(event) => setKeywordDraft(event.target.value)}
              />
            </DataTableFilterField>
            <DataTableFilterField label={t('origin')}>
              <Select
                allowClear
                aria-label={t('component_filter_origin')}
                value={originDraft}
                onChange={setOriginDraft}
                options={(['official', 'custom'] as const).map((value) => ({
                  value,
                  label: t(value)
                }))}
              />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
        <DataTable
          columns={columns}
          configuration={tableConfiguration}
          dataSource={visible}
          loading={query.isLoading || query.isFetching}
          page={page}
          pageSize={PAGE_SIZE}
          rowKey="id"
          total={filtered.length}
          onPageChange={setPage}
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <Button onClick={() => setCatalogOpen(true)}>
                {t('remote_catalog')}
              </Button>
              {canManage ? (
                <Button type="primary" onClick={create}>
                  {t('new_component')}
                </Button>
              ) : null}
              <Button onClick={() => query.refetch()}>
                {t('component_refresh')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
        />
      </DataTableLayout>

      <ResizableDrawer
        defaultWidth={720}
        open={drawerMode !== undefined}
        resizeLabel={t('resize_component_drawer')}
        title={
          drawerMode === 'create' ? t('new_component') : (selected?.name ?? '')
        }
        onClose={closeDrawer}
        extra={
          !readOnly ? (
            <Button
              type="primary"
              loading={save.isPending}
              onClick={() => form.submit()}
            >
              {t('save_component')}
            </Button>
          ) : undefined
        }
      >
        {readOnly && selected ? (
          <>
            <Typography.Paragraph strong>
              {t(
                selected.origin === 'official'
                  ? 'official_read_only'
                  : 'custom_component'
              )}
            </Typography.Paragraph>
            <Descriptions
              column={1}
              size="small"
              bordered
              items={[
                { key: 'id', label: t('id'), children: selected.id },
                {
                  key: 'scope_id',
                  label: t('scope_id'),
                  children: selected.scope_id
                },
                {
                  key: 'component_code',
                  label: t('component_code'),
                  children: selected.component_code
                },
                {
                  key: 'description',
                  label: t('description'),
                  children: selected.description
                },
                {
                  key: 'source',
                  label: t('source'),
                  children: selected.source
                },
                { key: 'group', label: t('group'), children: selected.group },
                {
                  key: 'upstream',
                  label: t('upstream_identity'),
                  children: `${selected.upstream.identity} @ ${selected.upstream.version}`
                },
                {
                  key: 'version',
                  label: t('version'),
                  children: selected.version
                },
                {
                  key: 'timestamps',
                  label: t('timestamps'),
                  children: `${selected.created_at} / ${selected.updated_at}`
                }
              ]}
            />
            <Typography.Title level={5}>{t('import_code')}</Typography.Title>
            <BlockSourceEditor
              ariaLabel={t('import_code')}
              height={180}
              path={`file:///ui-components/${selected.component_code}.imports.tsx`}
              readOnly
              value={selected.import_code}
              onChange={() => undefined}
            />
            <Typography.Title level={5}>{t('source_code')}</Typography.Title>
            <BlockSourceEditor
              ariaLabel={t('source_code')}
              height={240}
              path={`file:///ui-components/${selected.component_code}.tsx`}
              readOnly
              value={selected.source_code}
              onChange={() => undefined}
            />
          </>
        ) : (
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
              <Input disabled={drawerMode === 'edit'} />
            </Form.Item>
            <Form.Item
              name="name"
              label={t('name')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="description"
              label={t('description')}
              rules={[{ required: true }]}
            >
              <Input.TextArea rows={3} />
            </Form.Item>
            <Form.Item
              name="source"
              label={t('source')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="group"
              label={t('group')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name={['upstream', 'identity']}
              label={t('upstream_identity')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name={['upstream', 'version']}
              label={t('upstream_version')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="version"
              label={t('version')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item name="keywords" label={t('keywords')}>
              <Input />
            </Form.Item>
            <Form.Item
              name="import_code"
              label={t('import_code')}
              rules={[{ required: true }]}
            >
              <BlockSourceEditor
                ariaLabel={t('import_code')}
                height={180}
                path="file:///ui-components/imports.tsx"
                value={importCode}
                onChange={(value) => form.setFieldValue('import_code', value)}
              />
            </Form.Item>
            <Form.Item
              name="source_code"
              label={t('source_code')}
              rules={[{ required: true }]}
            >
              <BlockSourceEditor
                ariaLabel={t('source_code')}
                height={240}
                path="file:///ui-components/source.tsx"
                value={sourceCode}
                onChange={(value) => form.setFieldValue('source_code', value)}
              />
            </Form.Item>
          </Form>
        )}
      </ResizableDrawer>

      <Modal
        open={deleteTarget !== undefined}
        title={t('delete_component')}
        okText={t('confirm_delete')}
        okButtonProps={{ danger: true, loading: remove.isPending }}
        cancelText={t('cancel')}
        onCancel={() => setDeleteTarget(undefined)}
        onOk={() => deleteTarget && remove.mutate(deleteTarget)}
      >
        {deleteTarget
          ? t('delete_component_confirmation', { name: deleteTarget.name })
          : null}
      </Modal>
      <RemoteCatalogDrawer
        canManage={canManage}
        open={catalogOpen}
        onClose={() => setCatalogOpen(false)}
      />
    </SettingsSectionSurface>
  );
}
