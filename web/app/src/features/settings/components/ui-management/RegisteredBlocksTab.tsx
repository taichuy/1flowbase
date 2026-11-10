import { useCallback, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Descriptions,
  Flex,
  Input,
  Space,
  Tag,
  Tooltip
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { selectNavigationQueryScope } from '../../../../state/navigation-query-scope';
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
  fetchSettingsUiBlocks,
  settingsUiBlocksQueryKey,
  fetchSettingsCustomBlocks,
  createSettingsCustomBlock,
  updateSettingsCustomBlock,
  deleteSettingsCustomBlock,
  type ConsoleFrontendBlockCatalogEntry,
  type ConsoleCustomFrontendBlock,
  type SettingsUiTemplateInput
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import {
  UiCodeTemplateStudio,
  type UiCodeTemplateStudioMode
} from './UiCodeTemplateStudio';

const PAGE_SIZE = 20;
type BlockRow =
  | { kind: 'native'; record: ConsoleFrontendBlockCatalogEntry }
  | { kind: 'custom'; record: ConsoleCustomFrontendBlock };
type StudioSession = {
  documentId: string;
  mode: UiCodeTemplateStudioMode;
  id?: string;
  initialValue: SettingsUiTemplateInput;
};

export function RegisteredBlocksTab() {
  const scope = useAuthStore(selectNavigationQueryScope);
  return <RegisteredBlocksContent key={scope} scope={scope} />;
}

function RegisteredBlocksContent({ scope }: { scope: string }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message, modal } = App.useApp();
  const can = useConsoleOperationAccess();
  const canView = can('frontend_blocks.view');
  const canListCustom = can('ui_management.blocks.list');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const queryClient = useQueryClient();
  const [page, setPage] = useState(1);
  const [keywordDraft, setKeywordDraft] = useState('');
  const [keyword, setKeyword] = useState('');
  const [studio, setStudio] = useState<StudioSession | null>(null);
  const [detail, setDetail] = useState<ConsoleFrontendBlockCatalogEntry | null>(
    null
  );
  const query = useQuery({
    queryKey: [...settingsUiBlocksQueryKey, 'catalog', 'block', scope],
    queryFn: () =>
      fetchSettingsUiBlocks(undefined, { registration_target: 'block' }),
    enabled: canView,
    retry: false
  });
  const custom = useQuery({
    queryKey: [...settingsUiBlocksQueryKey, 'custom', scope],
    queryFn: () => fetchSettingsCustomBlocks(),
    enabled: canListCustom,
    retry: false
  });
  const invalidate = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: settingsUiBlocksQueryKey }),
      queryClient.invalidateQueries({
        queryKey: ['frontstage', 'block-catalog']
      })
    ]);
  }, [queryClient]);
  const save = useMutation({
    mutationFn: async (value: SettingsUiTemplateInput) => {
      if (!csrfToken || !studio) throw new Error(t('block_save_failed'));
      const body = {
        title: value.name,
        source: value.source,
        language: value.language
      };
      return studio.mode === 'edit' && studio.id
        ? updateSettingsCustomBlock(studio.id, body, csrfToken)
        : createSettingsCustomBlock(
            {
              ...body,
              provider_code: value.provider_code,
              contribution_code: value.contribution_code
            },
            csrfToken
          );
    },
    onSuccess: async () => {
      setStudio(null);
      await invalidate();
      void message.success(t('block_saved'));
    },
    onError: () => void message.error(t('block_save_failed'))
  });
  const remove = useMutation({
    mutationFn: async (id: string) => {
      if (!csrfToken) throw new Error(t('block_delete_failed'));
      await deleteSettingsCustomBlock(id, csrfToken);
    },
    onSuccess: async () => {
      await invalidate();
      void message.success(t('block_deleted'));
    },
    onError: () => void message.error(t('block_delete_failed'))
  });
  const open = useCallback(
    (row: BlockRow, mode: 'view' | 'edit' | 'copy') => {
      const source =
        row.kind === 'native' ? row.record.code_template : row.record.source;
      const language =
        row.kind === 'native'
          ? row.record.code_template_language
          : row.record.language;
      if (!source || !language) {
        if (row.kind === 'native' && mode === 'view') setDetail(row.record);
        return;
      }
      setStudio({
        documentId:
          mode === 'copy'
            ? `copy:${crypto.randomUUID()}`
            : row.kind === 'custom'
              ? row.record.id
              : row.record.frontend_contribution_id,
        mode,
        id:
          row.kind === 'custom' && mode !== 'copy' ? row.record.id : undefined,
        initialValue: {
          provider_code: row.record.provider_code,
          contribution_code: row.record.contribution_code,
          name:
            mode === 'copy'
              ? `${row.record.title} - ${t('copy_name_suffix')}`
              : row.record.title,
          source,
          language
        }
      });
    },
    [t]
  );
  const rows = useMemo<BlockRow[]>(
    () => [
      ...(query.data ?? [])
        .filter((record) => !record.custom_block_id)
        .map((record) => ({ kind: 'native' as const, record })),
      ...(custom.data ?? []).map((record) => ({
        kind: 'custom' as const,
        record
      }))
    ],
    [query.data, custom.data]
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
        title: t('block_version'),
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
                  query.data?.some(
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
                onClick={() => open(row, 'view')}
              >
                {t('view')}
              </Button>
              {row.kind === 'custom' ? (
                <Button
                  type="text"
                  size="small"
                  disabled={!can('ui_management.blocks.update')}
                  onClick={() => open(row, 'edit')}
                >
                  {t('edit')}
                </Button>
              ) : null}
              <Tooltip title={!copyable ? t('block_not_copyable') : undefined}>
                <Button
                  type="text"
                  size="small"
                  disabled={!copyable || !can('ui_management.blocks.create')}
                  onClick={() => open(row, 'copy')}
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
                    remove.isPending
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
                      onOk: () => remove.mutateAsync(row.record.id)
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
    [t, can, open, modal, csrfToken, remove, query.data]
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
  const failed = query.isError || custom.isError;
  const refresh = () => {
    if (canView) void query.refetch();
    if (canListCustom) void custom.refetch();
  };
  return (
    <SettingsSectionSurface
      heightMode="fill"
      status={
        !canView && !canListCustom ? (
          <Alert type="warning" showIcon title={t('blocks_no_access')} />
        ) : failed ? (
          <Alert
            type="error"
            showIcon
            title={t('blocks_load_failed')}
            action={
              <Button
                loading={query.isFetching || custom.isFetching}
                onClick={refresh}
              >
                {t('routes_retry')}
              </Button>
            }
          />
        ) : undefined
      }
    >
      {canView || canListCustom ? (
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
            loading={query.isFetching || custom.isFetching}
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
                <Button onClick={refresh}>{t('component_refresh')}</Button>
                <DataTableColumnSettings
                  columns={columns}
                  configuration={configuration}
                />
              </Flex>
            }
          />
        </DataTableLayout>
      ) : null}
      {studio ? (
        <UiCodeTemplateStudio
          resourceKind="block"
          documentId={studio.documentId}
          initialValue={studio.initialValue}
          mode={studio.mode}
          officialTemplates={[]}
          open
          saving={save.isPending}
          workspaceId={workspaceId}
          onClose={() => setStudio(null)}
          onSave={(value) => save.mutate(value)}
        />
      ) : null}
      <ResizableDrawer
        title={detail?.title}
        open={Boolean(detail)}
        onClose={() => setDetail(null)}
        defaultWidth={600}
        resizeLabel={t('block_resize')}
      >
        {detail ? (
          <Descriptions
            column={1}
            items={[
              {
                key: 'code',
                label: t('block_code'),
                children: detail.contribution_code
              },
              {
                key: 'source',
                label: t('source'),
                children: detail.provider_code
              },
              {
                key: 'version',
                label: t('block_version'),
                children: detail.frontend_block_version
              },
              {
                key: 'type',
                label: t('block_type'),
                children: t('block_native')
              }
            ]}
          />
        ) : null}
      </ResizableDrawer>
    </SettingsSectionSurface>
  );
}
