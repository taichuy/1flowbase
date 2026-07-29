import { ApiClientError } from '@1flowbase/api-client';
import { PlusOutlined, UndoOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Tag,
  message
} from 'antd';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
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

  const originLabels = useMemo<Record<SettingsI18nCatalogOrigin, string>>(
    () => ({
      official: t('auto.translation_catalog_origin_official'),
      official_override: t('auto.translation_catalog_origin_official_override'),
      custom: t('auto.translation_catalog_origin_custom'),
      english: t('auto.translation_catalog_origin_english')
    }),
    [t]
  );
  const originOptions = useMemo(
    () =>
      Object.entries(originLabels).map(([value, label]) => ({
        value: value as SettingsI18nCatalogOrigin,
        label
      })),
    [originLabels]
  );

  const columns = useMemo<Array<DataTableColumn<SettingsI18nCatalogEntry>>>(
    () => [
      {
        key: 'module',
        title: t('auto.translation_catalog_module'),
        dataIndex: 'module',
        width: 180,
        ellipsis: true
      },
      {
        key: 'msgid',
        title: t('auto.translation_catalog_msgid'),
        dataIndex: 'msgid',
        width: 240,
        ellipsis: true
      },
      {
        key: 'locale',
        title: t('auto.translation_catalog_locale'),
        dataIndex: 'locale',
        width: 120
      },
      {
        key: 'effective_value',
        title: t('auto.translation_catalog_effective_value'),
        dataIndex: 'effective_value',
        width: 320,
        ellipsis: true
      },
      {
        key: 'origin',
        title: t('auto.translation_catalog_origin'),
        dataIndex: 'origin',
        width: 160,
        render: (_, entry) => <Tag>{originLabels[entry.origin]}</Tag>
      },
      {
        key: 'status',
        title: t('auto.translation_catalog_status'),
        width: 180,
        render: (_, entry) => (
          <Space size={4} wrap>
            {entry.missing ? (
              <Tag color="error">{t('auto.translation_catalog_missing')}</Tag>
            ) : null}
            {entry.obsolete ? (
              <Tag color="warning">
                {t('auto.translation_catalog_obsolete')}
              </Tag>
            ) : null}
            {!entry.missing && !entry.obsolete ? (
              <Tag>{t('auto.translation_catalog_current')}</Tag>
            ) : null}
          </Space>
        )
      }
    ],
    [originLabels, t]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.i18n_catalog'
  });

  const entries = listQuery.data?.entries ?? [];
  const revision = listQuery.data?.revision ?? 0;

  return (
    <SettingsSectionSurface
      heightMode="fill"
      toolbar={
        <Form
          className="i18n-catalog-page__toolbar-form"
          form={filterForm}
          onFinish={(values) => {
            setFilters(values);
            setPage(1);
          }}
        >
          <Flex justify="space-between" align="flex-start" gap={12} wrap>
            <Flex className="i18n-catalog-page__filters" gap={8} wrap>
              <Form.Item
                className="i18n-catalog-page__filter-item i18n-catalog-page__filter-item--search"
                name="search"
              >
                <Input.Search
                  placeholder={t('auto.translation_catalog_search')}
                  onSearch={() => filterForm.submit()}
                  allowClear
                  data-testid="i18n-catalog-search"
                />
              </Form.Item>
              <Form.Item
                className="i18n-catalog-page__filter-item"
                name="module"
              >
                <Input
                  placeholder={t('auto.translation_catalog_module')}
                  allowClear
                  data-testid="i18n-catalog-module-filter"
                />
              </Form.Item>
              <Form.Item
                className="i18n-catalog-page__filter-item i18n-catalog-page__filter-item--compact"
                name="locale"
              >
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
              <Form.Item
                className="i18n-catalog-page__filter-item"
                name="origin"
              >
                <Select
                  allowClear
                  placeholder={t('auto.translation_catalog_origin')}
                  options={originOptions}
                  data-testid="i18n-catalog-origin-filter"
                />
              </Form.Item>
            </Flex>
            <Space className="i18n-catalog-page__actions" wrap>
              <Button
                className="i18n-catalog-page__filter-submit"
                htmlType="submit"
                data-testid="i18n-catalog-apply-filters"
              >
                {t('auto.translation_catalog_apply_filters')}
              </Button>
              <Button
                className="i18n-catalog-page__action"
                icon={<UndoOutlined />}
                onClick={() => setRestoreAllOpen(true)}
              >
                {t('auto.translation_catalog_restore_defaults')}
              </Button>
              <Button
                className="i18n-catalog-page__action"
                type="primary"
                icon={<PlusOutlined />}
                onClick={() => setCreateOpen(true)}
              >
                {t('auto.translation_catalog_create_action')}
              </Button>
            </Space>
          </Flex>
        </Form>
      }
    >
      {messageContextHolder}
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
      {listQuery.isError ? (
        <Alert
          type="error"
          showIcon
          message={t('auto.translation_catalog_load_failed')}
        />
      ) : null}
      <div
        className="i18n-catalog-page"
        data-testid="i18n-catalog-page"
        data-ready={!listQuery.isLoading}
      >
        <div
          className="i18n-catalog-page__table-region"
          data-testid="i18n-catalog-table"
        >
          <DataTable<SettingsI18nCatalogEntry>
            columns={columns}
            configuration={tableConfiguration}
            dataSource={entries}
            loading={listQuery.isLoading}
            page={page}
            pageSize={PAGE_SIZE}
            total={listQuery.data?.total ?? 0}
            rowKey={(entry) => `${entry.module}:${entry.msgid}:${entry.locale}`}
            onRow={(entry) => ({
              onClick: () => setSelectedIdentity(identityOf(entry))
            })}
            onPageChange={setPage}
          />
        </div>
      </div>
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
          deleteEntry && mutation.mutate({ kind: 'delete', entry: deleteEntry })
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
    </SettingsSectionSurface>
  );
}
