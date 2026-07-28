import { ApiClientError } from '@1flowbase/api-client';
import { PlusOutlined, UndoOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Empty,
  Flex,
  Form,
  Input,
  List,
  Modal,
  Pagination,
  Select,
  Space,
  Table,
  Tag,
  Typography,
  message,
  type TableProps
} from 'antd';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  deleteSettingsCustomI18nCatalogKey,
  fetchSettingsI18nCatalogEntries,
  fetchSettingsI18nCatalogEntry,
  restoreAllSettingsI18nCatalogOverrides,
  restoreSettingsI18nCatalogOverride,
  saveSettingsCustomI18nCatalogTranslation,
  saveSettingsI18nCatalogOverride,
  settingsI18nCatalogEntryQueryKey,
  settingsI18nCatalogListQueryKey,
  settingsI18nCatalogQueryKey,
  type SettingsI18nCatalogEntry,
  type SettingsI18nCatalogListRequest,
  type SettingsI18nCatalogOrigin
} from '../../api/i18n-catalog';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import {
  I18nCatalogCreateDrawer,
  type CreateCustomTranslationValues
} from './I18nCatalogCreateDrawer';
import { I18nCatalogEntryDrawer } from './I18nCatalogEntryDrawer';
import './i18n-catalog-page.css';

const PAGE_SIZE = 20;

interface CatalogFilters {
  module?: string;
  locale?: string;
  search?: string;
  origin?: SettingsI18nCatalogOrigin;
}

interface CatalogIdentity {
  module: string;
  msgid: string;
  locale: string;
}

type CatalogAction =
  | { kind: 'save'; entry: SettingsI18nCatalogEntry; translation: string }
  | { kind: 'restore'; entry: SettingsI18nCatalogEntry }
  | { kind: 'delete'; entry: SettingsI18nCatalogEntry }
  | { kind: 'restore-all'; revision: number }
  | { kind: 'create'; values: CreateCustomTranslationValues; revision: number };

function identityOf(entry: SettingsI18nCatalogEntry): CatalogIdentity {
  return { module: entry.module, msgid: entry.msgid, locale: entry.locale };
}

export function I18nCatalogPage() {
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [messageApi, messageContextHolder] = message.useMessage();
  const [filterForm] = Form.useForm<CatalogFilters>();
  const [filters, setFilters] = useState<CatalogFilters>({});
  const [page, setPage] = useState(1);
  const [selectedIdentity, setSelectedIdentity] =
    useState<CatalogIdentity | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteEntry, setDeleteEntry] =
    useState<SettingsI18nCatalogEntry | null>(null);
  const [restoreAllOpen, setRestoreAllOpen] = useState(false);
  const [conflictVisible, setConflictVisible] = useState(false);

  const listRequest: SettingsI18nCatalogListRequest = useMemo(
    () => ({ ...filters, offset: (page - 1) * PAGE_SIZE, limit: PAGE_SIZE }),
    [filters, page]
  );
  const listQuery = useQuery({
    queryKey: settingsI18nCatalogListQueryKey(listRequest),
    queryFn: () => fetchSettingsI18nCatalogEntries(listRequest)
  });
  const detailQuery = useQuery({
    queryKey: selectedIdentity
      ? settingsI18nCatalogEntryQueryKey(selectedIdentity)
      : [...settingsI18nCatalogQueryKey, 'entry', 'closed'],
    queryFn: () => fetchSettingsI18nCatalogEntry(selectedIdentity!),
    enabled: selectedIdentity !== null
  });

  const refreshCatalog = async () => {
    await queryClient.invalidateQueries({
      queryKey: settingsI18nCatalogQueryKey
    });
  };

  const mutation = useMutation({
    mutationFn: async (action: CatalogAction) => {
      if (!csrfToken) throw new Error('missing csrf token');
      switch (action.kind) {
        case 'save': {
          const request = {
            ...identityOf(action.entry),
            translation: action.translation,
            expected_revision: action.entry.revision
          };
          return action.entry.origin === 'custom'
            ? saveSettingsCustomI18nCatalogTranslation(request, csrfToken)
            : saveSettingsI18nCatalogOverride(request, csrfToken);
        }
        case 'restore':
          return restoreSettingsI18nCatalogOverride(
            {
              ...identityOf(action.entry),
              expected_revision: action.entry.revision
            },
            csrfToken
          );
        case 'delete':
          return deleteSettingsCustomI18nCatalogKey(
            {
              module: action.entry.module,
              msgid: action.entry.msgid,
              expected_revision: action.entry.revision
            },
            csrfToken
          );
        case 'restore-all':
          return restoreAllSettingsI18nCatalogOverrides(
            { expected_revision: action.revision },
            csrfToken
          );
        case 'create':
          return saveSettingsCustomI18nCatalogTranslation(
            { ...action.values, expected_revision: action.revision },
            csrfToken
          );
      }
    },
    onSuccess: async (_, action) => {
      setConflictVisible(false);
      await refreshCatalog();
      if (action.kind === 'delete') setSelectedIdentity(null);
      if (action.kind === 'create') setCreateOpen(false);
      setDeleteEntry(null);
      setRestoreAllOpen(false);
      messageApi.success(t('auto.translation_catalog_change_saved'));
    },
    onError: async (error) => {
      if (error instanceof ApiClientError && error.status === 409) {
        setConflictVisible(true);
        await refreshCatalog();
        return;
      }
      messageApi.error(t('auto.translation_catalog_change_failed'));
    }
  });

  const originLabels: Record<SettingsI18nCatalogOrigin, string> = {
    official: t('auto.translation_catalog_origin_official'),
    official_override: t('auto.translation_catalog_origin_official_override'),
    custom: t('auto.translation_catalog_origin_custom'),
    english: t('auto.translation_catalog_origin_english')
  };
  const originOptions = Object.entries(originLabels).map(([value, label]) => ({
    value: value as SettingsI18nCatalogOrigin,
    label
  }));

  const statusTags = (entry: SettingsI18nCatalogEntry) => (
    <Space size={4} wrap>
      {entry.missing ? (
        <Tag color="error">{t('auto.translation_catalog_missing')}</Tag>
      ) : null}
      {entry.obsolete ? (
        <Tag color="warning">{t('auto.translation_catalog_obsolete')}</Tag>
      ) : null}
      {!entry.missing && !entry.obsolete ? (
        <Tag>{t('auto.translation_catalog_current')}</Tag>
      ) : null}
    </Space>
  );

  const columns: TableProps<SettingsI18nCatalogEntry>['columns'] = [
    {
      title: t('auto.translation_catalog_module'),
      dataIndex: 'module',
      width: 180,
      ellipsis: true
    },
    {
      title: t('auto.translation_catalog_msgid'),
      dataIndex: 'msgid',
      width: 220,
      ellipsis: true
    },
    {
      title: t('auto.translation_catalog_locale'),
      dataIndex: 'locale',
      width: 100
    },
    {
      title: t('auto.translation_catalog_effective_value'),
      dataIndex: 'effective_value',
      ellipsis: true
    },
    {
      title: t('auto.translation_catalog_origin'),
      dataIndex: 'origin',
      width: 150,
      render: (origin: SettingsI18nCatalogOrigin) => (
        <Tag>{originLabels[origin]}</Tag>
      )
    },
    {
      title: t('auto.translation_catalog_status'),
      width: 160,
      render: (_, entry) => statusTags(entry)
    }
  ];

  const entries = listQuery.data?.entries ?? [];
  const revision = listQuery.data?.revision ?? 0;

  return (
    <SettingsSectionSurface>
      {messageContextHolder}
      <div
        className="i18n-catalog-page"
        data-testid="i18n-catalog-page"
        data-ready={!listQuery.isLoading}
      >
        <Flex
          className="i18n-catalog-page__heading"
          justify="space-between"
          align="center"
          gap={12}
          wrap
        >
          <div>
            <Typography.Title level={3}>
              {t('auto.translation_catalog_title')}
            </Typography.Title>
            <Typography.Text type="secondary">
              {t('auto.translation_catalog_description')}
            </Typography.Text>
          </div>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            onClick={() => setCreateOpen(true)}
          >
            {t('auto.translation_catalog_create_custom_key')}
          </Button>
        </Flex>

        {conflictVisible ? (
          <Alert
            closable
            onClose={() => setConflictVisible(false)}
            showIcon
            type="warning"
            message={t('auto.translation_catalog_revision_conflict')}
            data-testid="i18n-catalog-conflict"
          />
        ) : null}

        <Form
          className="i18n-catalog-page__filters"
          form={filterForm}
          layout="inline"
          onFinish={(values) => {
            setFilters(values);
            setPage(1);
          }}
        >
          <Form.Item name="search">
            <Input.Search
              placeholder={t('auto.translation_catalog_search')}
              onSearch={() => filterForm.submit()}
              allowClear
              data-testid="i18n-catalog-search"
            />
          </Form.Item>
          <Form.Item name="module">
            <Input
              placeholder={t('auto.translation_catalog_module')}
              allowClear
              data-testid="i18n-catalog-module-filter"
            />
          </Form.Item>
          <Form.Item name="locale">
            <Select
              allowClear
              placeholder={t('auto.translation_catalog_locale')}
              data-testid="i18n-catalog-locale-filter"
              options={[
                { value: 'zh_Hans', label: 'zh_Hans' },
                { value: 'en_US', label: 'en_US' }
              ]}
            />
          </Form.Item>
          <Form.Item name="origin">
            <Select
              allowClear
              placeholder={t('auto.translation_catalog_origin')}
              options={originOptions}
              data-testid="i18n-catalog-origin-filter"
            />
          </Form.Item>
          <Button htmlType="submit" data-testid="i18n-catalog-apply-filters">
            {t('auto.translation_catalog_apply_filters')}
          </Button>
        </Form>

        <Flex
          className="i18n-catalog-page__status-line"
          justify="space-between"
          align="center"
          gap={8}
          wrap
        >
          <Typography.Text type="secondary">
            {t('auto.translation_catalog_summary', {
              total: listQuery.data?.total ?? 0,
              revision
            })}
          </Typography.Text>
          <Button
            icon={<UndoOutlined />}
            onClick={() => setRestoreAllOpen(true)}
          >
            {t('auto.translation_catalog_restore_all')}
          </Button>
        </Flex>

        {listQuery.isError ? (
          <Alert
            type="error"
            showIcon
            message={t('auto.translation_catalog_load_failed')}
          />
        ) : null}

        <div
          className="i18n-catalog-page__desktop"
          data-testid="i18n-catalog-desktop-table"
        >
          <Table
            columns={columns}
            dataSource={entries}
            loading={listQuery.isLoading}
            pagination={false}
            rowKey={(entry) => `${entry.module}:${entry.msgid}:${entry.locale}`}
            onRow={(entry) => ({
              onClick: () => setSelectedIdentity(identityOf(entry))
            })}
            size="small"
          />
        </div>

        <div
          className="i18n-catalog-page__mobile"
          data-testid="i18n-catalog-mobile-list"
        >
          <List
            dataSource={entries}
            loading={listQuery.isLoading}
            locale={{
              emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
            }}
            renderItem={(entry) => (
              <List.Item
                className="i18n-catalog-page__mobile-row"
                onClick={() => setSelectedIdentity(identityOf(entry))}
              >
                <List.Item.Meta
                  title={
                    <Space wrap>
                      <Typography.Text code>{entry.msgid}</Typography.Text>
                      <Tag>{entry.locale}</Tag>
                    </Space>
                  }
                  description={
                    <Space direction="vertical" size={4}>
                      <Typography.Text type="secondary">
                        {entry.module}
                      </Typography.Text>
                      <Typography.Text>{entry.effective_value}</Typography.Text>
                      <Space wrap>
                        <Tag>{originLabels[entry.origin]}</Tag>
                        {statusTags(entry)}
                      </Space>
                    </Space>
                  }
                />
              </List.Item>
            )}
          />
        </div>

        <Pagination
          current={page}
          pageSize={PAGE_SIZE}
          total={listQuery.data?.total ?? 0}
          showSizeChanger={false}
          onChange={setPage}
        />

        <I18nCatalogEntryDrawer
          entry={detailQuery.data ?? null}
          loading={detailQuery.isLoading}
          open={selectedIdentity !== null}
          saving={mutation.isPending}
          onClose={() => setSelectedIdentity(null)}
          onSave={(translation) =>
            detailQuery.data &&
            mutation.mutate({
              kind: 'save',
              entry: detailQuery.data,
              translation
            })
          }
          onRestore={() =>
            detailQuery.data &&
            mutation.mutate({ kind: 'restore', entry: detailQuery.data })
          }
          onDelete={() => detailQuery.data && setDeleteEntry(detailQuery.data)}
        />
        <I18nCatalogCreateDrawer
          open={createOpen}
          saving={mutation.isPending}
          onClose={() => setCreateOpen(false)}
          onCreate={(values) =>
            mutation.mutate({ kind: 'create', values, revision })
          }
        />
        <Modal
          open={deleteEntry !== null}
          title={t('auto.translation_catalog_delete_custom_key')}
          okButtonProps={{ danger: true, loading: mutation.isPending }}
          okText={t('auto.translation_catalog_delete')}
          cancelText={t('auto.translation_catalog_cancel')}
          onCancel={() => setDeleteEntry(null)}
          onOk={() =>
            deleteEntry &&
            mutation.mutate({ kind: 'delete', entry: deleteEntry })
          }
          data-testid="i18n-catalog-delete-confirmation"
        >
          {t('auto.translation_catalog_delete_confirmation')}
        </Modal>
        <Modal
          open={restoreAllOpen}
          title={t('auto.translation_catalog_restore_all')}
          okText={t('auto.translation_catalog_restore')}
          cancelText={t('auto.translation_catalog_cancel')}
          confirmLoading={mutation.isPending}
          onCancel={() => setRestoreAllOpen(false)}
          onOk={() => mutation.mutate({ kind: 'restore-all', revision })}
          data-testid="i18n-catalog-restore-all-confirmation"
        >
          {t('auto.translation_catalog_restore_all_confirmation')}
        </Modal>
      </div>
    </SettingsSectionSurface>
  );
}
