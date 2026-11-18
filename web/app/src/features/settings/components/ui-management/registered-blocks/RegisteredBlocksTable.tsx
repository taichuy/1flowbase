import { useMemo, useState } from 'react';
import { App, Button, Flex, Input, Space, Tag, Tooltip } from 'antd';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../../state/console-operation-access';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../../shared/ui/data-table/DataTable';
import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../../shared/ui/data-table/data-table-state';
import type {
  ConsoleFrontendBlockCatalogEntry,
  ConsoleCustomFrontendBlock
} from '../../../api/ui-management';

const PAGE_SIZE = 20;
export type BlockRow =
  | { kind: 'native'; record: ConsoleFrontendBlockCatalogEntry }
  | { kind: 'custom'; record: ConsoleCustomFrontendBlock };

export function RegisteredBlocksTable({
  catalog,
  custom,
  loading,
  deleting,
  onOpen,
  onDelete,
  onRefresh
}: {
  catalog: ConsoleFrontendBlockCatalogEntry[];
  custom: ConsoleCustomFrontendBlock[];
  loading: boolean;
  deleting: boolean;
  onOpen: (row: BlockRow, mode: 'view' | 'edit' | 'copy') => void;
  onDelete: (id: string) => Promise<unknown>;
  onRefresh: () => void;
}) {
  const { t } = useTranslation('settingsUiManagement');
  const { modal } = App.useApp();
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [page, setPage] = useState(1);
  const [keywordDraft, setKeywordDraft] = useState('');
  const [keyword, setKeyword] = useState('');
  const rows = useMemo<BlockRow[]>(
    () => [
      ...catalog.flatMap((record) =>
        record.custom_block_id ? [] : [{ kind: 'native' as const, record }]
      ),
      ...custom.map((record) => ({
        kind: 'custom' as const,
        record
      }))
    ],
    [catalog, custom]
  );
  const columns = useMemo<Array<DataTableColumn<BlockRow>>>(
    () => [
      {
        key: 'title',
        title: t('name'),
        render: (_, row) => row.record.title,
        width: 280,
        sizing: 'fill',
        ellipsis: true,
        mobileRole: 'title'
      },
      {
        key: 'contribution_code',
        title: t('block_code'),
        width: 260,
        ellipsis: true,
        render: (_, row) =>
          row.kind === 'custom' ? row.record.id : row.record.contribution_code
      },
      {
        key: 'provider_code',
        title: t('source'),
        width: 160,
        render: (_, row) =>
          row.kind === 'custom' ? t('custom') : row.record.provider_code
      },
      {
        key: 'frontend_block_version',
        title: t('resource_version'),
        width: 100,
        render: (_, row) =>
          row.kind === 'custom'
            ? `r${row.record.revision}`
            : row.record.frontend_block_version
      },
      {
        key: 'runtime_kind',
        title: t('block_type'),
        width: 110,
        render: (_, row) =>
          row.kind === 'native' ? t('block_native') : t('block_custom')
      },
      {
        key: 'disable_reason',
        title: t('status'),
        width: 100,
        render: (_, row) => {
          const available =
            row.kind === 'native'
              ? row.record.disable_reason === null
              : Boolean(
                  catalog.some(
                    (entry) =>
                      entry.custom_block_id === row.record.id &&
                      entry.disable_reason === null
                  )
                );
          return (
            <Tag color={available ? 'success' : 'default'}>
              {available ? t('block_available') : t('block_unavailable')}
            </Tag>
          );
        }
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 240,
        minWidth: 240,
        mobileRole: 'actions',
        render: (_, row) => {
          const copyable =
            row.kind === 'custom' ||
            Boolean(
              row.record.code_template && row.record.code_template_language
            );
          return (
            <Space size={2}>
              <Button
                type="text"
                size="small"
                onClick={() => onOpen(row, 'view')}
              >
                {t('view')}
              </Button>
              {row.kind === 'custom' ? (
                <Button
                  type="text"
                  size="small"
                  disabled={!can('ui_management.blocks.update')}
                  onClick={() => onOpen(row, 'edit')}
                >
                  {t('edit')}
                </Button>
              ) : null}
              <Tooltip title={!copyable ? t('block_not_copyable') : undefined}>
                <Button
                  type="text"
                  size="small"
                  disabled={!copyable || !can('ui_management.blocks.create')}
                  onClick={() => onOpen(row, 'copy')}
                >
                  {t('copy')}
                </Button>
              </Tooltip>
              {row.kind === 'custom' ? (
                <Button
                  type="text"
                  size="small"
                  danger
                  disabled={
                    !can('ui_management.blocks.delete') ||
                    !csrfToken ||
                    deleting
                  }
                  onClick={() =>
                    modal.confirm({
                      title: t('delete_block_title'),
                      content: t('delete_block_description', {
                        name: row.record.title
                      }),
                      okText: t('delete'),
                      cancelText: t('cancel'),
                      okButtonProps: { danger: true },
                      onOk: () => onDelete(row.record.id)
                    })
                  }
                >
                  {t('delete')}
                </Button>
              ) : null}
            </Space>
          );
        }
      }
    ],
    [t, can, onOpen, modal, csrfToken, deleting, onDelete, catalog]
  );
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.ui_management.blocks'
  });
  const filtered = useMemo(() => {
    const search = keyword.trim().toLocaleLowerCase();
    return rows.filter((row) =>
      [
        row.record.title,
        row.record.contribution_code,
        row.record.provider_code,
        ...(row.kind === 'custom' ? [row.record.id, t('custom')] : [])
      ].some((value) => value.toLocaleLowerCase().includes(search))
    );
  }, [rows, keyword, t]);
  const currentPage = Math.min(
    page,
    Math.max(1, Math.ceil(filtered.length / PAGE_SIZE))
  );
  return (
    <DataTableLayout
      filters={
        <DataTableFilterForm
          ariaLabel={t('block_filter')}
          submitLabel={t('component_filter_submit')}
          resetLabel={t('component_filter_reset')}
          onSubmit={() => {
            setKeyword(keywordDraft);
            setPage(1);
          }}
          onReset={() => {
            setKeywordDraft('');
            setKeyword('');
            setPage(1);
          }}
        >
          <DataTableFilterField label={t('component_filter_keyword')}>
            <Input
              type="search"
              aria-label={t('block_search')}
              placeholder={t('block_search_hint')}
              value={keywordDraft}
              onChange={(event) => setKeywordDraft(event.target.value)}
            />
          </DataTableFilterField>
        </DataTableFilterForm>
      }
    >
      <DataTable
        columns={columns}
        configuration={configuration}
        dataSource={filtered.slice(
          (currentPage - 1) * PAGE_SIZE,
          currentPage * PAGE_SIZE
        )}
        loading={loading}
        emptyText={t('blocks_empty')}
        rowKey={(row) =>
          row.kind === 'native'
            ? row.record.frontend_contribution_id
            : row.record.id
        }
        page={currentPage}
        pageSize={PAGE_SIZE}
        total={filtered.length}
        onPageChange={setPage}
        toolbar={
          <Flex justify="flex-end" gap={8} wrap>
            <Button onClick={onRefresh}>{t('component_refresh')}</Button>
            <DataTableColumnSettings
              columns={columns}
              configuration={configuration}
            />
          </Flex>
        }
      />
    </DataTableLayout>
  );
}
