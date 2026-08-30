import { ApiClientError } from '@1flowbase/api-client';
import GlobalOutlined from '@ant-design/icons/es/icons/GlobalOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import UndoOutlined from '@ant-design/icons/es/icons/UndoOutlined';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Flex,
  Input,
  Modal,
  Select,
  Space,
  Tag,
  App
} from 'antd';
import { useCallback, useMemo, useState } from 'react';
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
import { I18nCatalogActivationFlow } from '../../components/i18n-catalog/I18nCatalogActivationFlow';
import {
  I18nCatalogCreateDrawer,
  type CreateCustomTranslationValues
} from './I18nCatalogCreateDrawer';
import { I18nCatalogEntryDrawer } from './I18nCatalogEntryDrawer';
import './i18n-catalog-page.css';

const PAGE_SIZE = 20;

interface CatalogFilters {
  locale?: string;
  search?: string;
  origin?: SettingsI18nCatalogOrigin;
}

interface CatalogIdentity {
  key: string;
  locale: string;
}

type CatalogAction =
  | { kind: 'save'; entry: SettingsI18nCatalogEntry; translation: string }
  | { kind: 'restore'; entry: SettingsI18nCatalogEntry }
  | { kind: 'delete'; entry: SettingsI18nCatalogEntry }
  | { kind: 'restore-all'; revision: number }
  | { kind: 'create'; values: CreateCustomTranslationValues; revision: number };

function identityOf(entry: SettingsI18nCatalogEntry): CatalogIdentity {
  return { key: entry.key, locale: entry.locale };
}

export function I18nCatalogPage() {
  const { t } = useTranslation('settings');
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const { message: messageApi } = App.useApp();
  const [filterDraft, setFilterDraft] = useState<CatalogFilters>({});
  const [filters, setFilters] = useState<CatalogFilters>({});
  const [page, setPage] = useState(1);
  const [selectedIdentity, setSelectedIdentity] =
    useState<CatalogIdentity | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteEntry, setDeleteEntry] =
    useState<SettingsI18nCatalogEntry | null>(null);
  const [restoreAllOpen, setRestoreAllOpen] = useState(false);
  const [catalogActivationOpen, setCatalogActivationOpen] = useState(false);
  const [conflictVisible, setConflictVisible] = useState(false);
  const closeCatalogActivation = useCallback(
    () => setCatalogActivationOpen(false),
    []
  );

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
              key: action.entry.key,
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
        key: 'key',
        title: t('auto.key'),
        dataIndex: 'key',
        width: 320,
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
        title: t('auto.status'),
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

  function applyFilters() {
    const nextFilters = {
      ...filterDraft,
      search: filterDraft.search?.trim() || undefined
    };
    setFilterDraft(nextFilters);
    setFilters(nextFilters);
    setPage(1);
  }

  function resetFilters() {
    setFilterDraft({});
    setFilters({});
    setPage(1);
  }

  return (
    <SettingsSectionSurface heightMode="fill">
      {conflictVisible ? (
        <Alert
          closable
          onClose={() => setConflictVisible(false)}
          showIcon
          type="warning"
          title={t('auto.translation_catalog_revision_conflict')}
          data-testid="i18n-catalog-conflict"
        />
      ) : null}
      {listQuery.isError ? (
        <Alert
          type="error"
          showIcon
          title={t('auto.translation_catalog_load_failed')}
        />
      ) : null}
      <DataTableLayout
        filters={
          <DataTableFilterForm
            ariaLabel={t('auto.translation_catalog_filter')}
            resetLabel={t('auto.reset')}
            submitLabel={t('auto.translation_catalog_filter')}
            onReset={resetFilters}
            onSubmit={applyFilters}
          >
            <DataTableFilterField label={t('auto.translation_catalog_search')}>
              <Input.Search
                allowClear
                aria-label={t('auto.translation_catalog_search')}
                data-testid="i18n-catalog-search"
                placeholder={t('auto.translation_catalog_search')}
                value={filterDraft.search}
                onChange={(event) =>
                  setFilterDraft((current) => ({
                    ...current,
                    search: event.target.value
                  }))
                }
                onSearch={applyFilters}
              />
            </DataTableFilterField>
            <DataTableFilterField label={t('auto.translation_catalog_locale')}>
              <Select
                allowClear
                aria-label={t('auto.translation_catalog_locale')}
                data-testid="i18n-catalog-locale-filter"
                placeholder={t('auto.translation_catalog_locale')}
                value={filterDraft.locale}
                options={[
                  { value: 'zh_Hans', label: 'zh_Hans' },
                  { value: 'en_US', label: 'en_US' }
                ]}
                onChange={(locale) =>
                  setFilterDraft((current) => ({ ...current, locale }))
                }
              />
            </DataTableFilterField>
            <DataTableFilterField label={t('auto.translation_catalog_origin')}>
              <Select
                allowClear
                aria-label={t('auto.translation_catalog_origin')}
                data-testid="i18n-catalog-origin-filter"
                placeholder={t('auto.translation_catalog_origin')}
                value={filterDraft.origin}
                options={originOptions}
                onChange={(origin) =>
                  setFilterDraft((current) => ({ ...current, origin }))
                }
              />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
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
              rowKey={(entry) => `${entry.key}:${entry.locale}`}
              toolbar={
                <Flex justify="flex-end" gap={8} wrap>
                  <Button
                    icon={<GlobalOutlined />}
                    aria-label={t('auto.translation_catalog_version')}
                    onClick={() => setCatalogActivationOpen(true)}
                  >
                    {t('auto.translation_catalog_version')}
                  </Button>
                  <Button
                    icon={<UndoOutlined />}
                    onClick={() => setRestoreAllOpen(true)}
                  >
                    {t('auto.translation_catalog_restore_defaults')}
                  </Button>
                  <Button
                    type="primary"
                    icon={<PlusOutlined />}
                    onClick={() => setCreateOpen(true)}
                  >
                    {t('auto.new')}
                  </Button>
                  <DataTableColumnSettings
                    columns={columns}
                    configuration={tableConfiguration}
                  />
                </Flex>
              }
              onRow={(entry) => ({
                onClick: () => setSelectedIdentity(identityOf(entry))
              })}
              onPageChange={setPage}
            />
          </div>
        </div>
      </DataTableLayout>
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
      <I18nCatalogActivationFlow
        source={catalogActivationOpen ? { kind: 'official' } : null}
        csrfToken={csrfToken ?? ''}
        onClose={closeCatalogActivation}
        onActivated={refreshCatalog}
      />
    </SettingsSectionSurface>
  );
}
