import { useCallback, useMemo, useState, type HTMLAttributes } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, App, Button, Flex, Space, Tag } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';

import { useAuthStore } from '../../../../state/auth-store';
import {
  deleteSettingsUiTemplate,
  createSettingsUiTemplate,
  fetchSettingsUiTemplates,
  publishSettingsUiTemplate,
  resetSettingsUiTemplateDefault,
  setSettingsUiTemplateDefault,
  settingsUiTemplatesQueryKey,
  updateSettingsUiTemplate,
  type SettingsUiManagedTemplate,
  type SettingsUiOfficialTemplate,
  type SettingsUiTemplateInput
} from '../../api/ui-management';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import {
  UiCodeTemplateStudio,
  type UiCodeTemplateStudioMode
} from './UiCodeTemplateStudio';

type StudioSession = {
  mode: UiCodeTemplateStudioMode;
  templateId: string | null;
  managedTemplate?: SettingsUiManagedTemplate;
  initialValue: SettingsUiTemplateInput;
};

type OfficialRow = SettingsUiOfficialTemplate & {
  key: string;
  kind: 'official';
  name: string;
  revision: string;
  status: 'published';
};

type ManagedRow = SettingsUiManagedTemplate & {
  key: string;
  kind: 'managed';
  revision: string;
  status: 'published' | 'draft';
};

type TemplateRow = OfficialRow | ManagedRow;

function requireToken(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

export function CodeTemplatesTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const { message, modal } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const queryClient = useQueryClient();
  // Capture the editor's starting revision when it opens. Background registration/template
  // refreshes update the table, but must not replace an open editor's unsaved source.
  const [studio, setStudio] = useState<StudioSession | null>(null);
  const query = useQuery({
    queryKey: settingsUiTemplatesQueryKey,
    queryFn: () => fetchSettingsUiTemplates()
  });
  const save = useMutation({
    mutationFn: async (value: SettingsUiTemplateInput) => {
      if (!studio) throw new Error('missing template studio session');
      return studio.mode === 'edit' && studio.templateId
        ? updateSettingsUiTemplate(
            studio.templateId,
            {
              name: value.name,
              source: value.source,
              language: value.language
            },
            requireToken(csrfToken)
          )
        : createSettingsUiTemplate(value, requireToken(csrfToken));
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsUiTemplatesQueryKey
      });
      setStudio(null);
      void message.success(t('saved'));
    },
    onError: (error) =>
      void message.error(
        error instanceof Error ? error.message : t('template_save_failed')
      )
  });
  const action = useMutation({
    mutationFn: async (run: () => Promise<unknown>) => run(),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsUiTemplatesQueryKey
        }),
        queryClient.invalidateQueries({
          predicate: ({ queryKey }) =>
            queryKey[0] === 'frontstage' && queryKey[2] === 'ui-templates'
        })
      ]);
    },
    onError: (error) =>
      void message.error(
        error instanceof Error ? error.message : t('template_action_failed')
      )
  });
  const officialTemplates = query.data?.official ?? [];
  const rows = useMemo<TemplateRow[]>(
    () => [
      ...(query.data?.official.map((row) => ({
        key: `official:${row.provider_code}:${row.contribution_code}`,
        kind: 'official' as const,
        ...row,
        name: row.title,
        revision: row.version,
        status: 'published' as const
      })) ?? []),
      ...(query.data?.managed.map((row) => ({
        key: row.id,
        kind: 'managed' as const,
        ...row,
        revision: `r${row.latest_revision.revision}`,
        status: row.published_revision
          ? ('published' as const)
          : ('draft' as const)
      })) ?? [])
    ],
    [query.data]
  );

  const openCreate = () => {
    const template = query.data?.default_template;
    setStudio({
      mode: 'create',
      templateId: null,
      initialValue: {
        provider_code: template?.provider_code ?? '',
        contribution_code: template?.contribution_code ?? '',
        name: '',
        source: template?.source ?? '',
        language: template?.language ?? 'tsx'
      }
    });
  };
  const openOfficial = useCallback(
    (row: OfficialRow, mode: 'view' | 'copy') =>
      setStudio({
        mode,
        templateId: null,
        initialValue: {
          provider_code: row.provider_code,
          contribution_code: row.contribution_code,
          name:
            mode === 'copy'
              ? `${row.title} - ${t('copy_name_suffix')}`
              : row.title,
          source: row.source,
          language: row.language
        }
      }),
    [t]
  );
  const openManaged = useCallback(
    (row: ManagedRow, mode: 'edit' | 'copy') =>
      setStudio({
        mode,
        templateId: mode === 'edit' ? row.id : null,
        managedTemplate: mode === 'edit' ? row : undefined,
        initialValue: {
          provider_code: row.provider_code,
          contribution_code: row.contribution_code,
          name:
            mode === 'copy'
              ? `${row.name} - ${t('copy_name_suffix')}`
              : row.name,
          source: row.latest_revision.source,
          language: row.latest_revision.language
        }
      }),
    [t]
  );

  const {
    mutate: runAction,
    mutateAsync: runActionAsync,
    isPending: actionPending
  } = action;
  const columns = useMemo<Array<DataTableColumn<TemplateRow>>>(
    () => [
      {
        key: 'name',
        title: t('name'),
        dataIndex: 'name',
        width: 240,
        minWidth: 180,
        sizing: 'fill',
        ellipsis: true,
        mobileRole: 'title'
      },
      {
        key: 'contribution',
        title: t('contribution'),
        width: 300,
        ellipsis: true,
        render: (_, row) => `${row.provider_code}/${row.contribution_code}`
      },
      {
        key: 'source',
        title: t('source'),
        width: 110,
        render: (_, row) => (
          <Tag>{row.kind === 'official' ? t('official') : t('managed')}</Tag>
        )
      },
      {
        key: 'revision',
        title: t('revision'),
        dataIndex: 'revision',
        width: 100,
        minWidth: 80
      },
      {
        key: 'status',
        title: t('status'),
        width: 170,
        render: (_, row) => (
          <Space>
            <Tag color={row.status === 'published' ? 'green' : 'default'}>
              {t(row.status)}
            </Tag>
            {row.is_default ? <Tag color="blue">{t('default')}</Tag> : null}
          </Space>
        )
      },
      {
        key: 'actions',
        title: t('actions'),
        width: 340,
        minWidth: 340,
        mobileRole: 'actions',
        render: (_, row) => (
          <Space size={2}>
            {row.kind === 'official' ? (
              <Button
                type="text"
                size="small"
                onClick={() => openOfficial(row, 'view')}
              >
                {t('view')}
              </Button>
            ) : (
              <Button
                type="text"
                size="small"
                disabled={!canManage}
                onClick={() => openManaged(row, 'edit')}
              >
                {t('edit')}
              </Button>
            )}
            <Button
              type="text"
              size="small"
              disabled={!canManage}
              onClick={() =>
                row.kind === 'official'
                  ? openOfficial(row, 'copy')
                  : openManaged(row, 'copy')
              }
            >
              {t('copy')}
            </Button>
            {row.kind === 'managed' ? (
              <Button
                type="text"
                size="small"
                disabled={!canManage || row.latest_revision.is_published}
                onClick={() =>
                  runAction(() =>
                    publishSettingsUiTemplate(
                      row.id,
                      row.latest_revision.revision,
                      requireToken(csrfToken)
                    )
                  )
                }
              >
                {t('publish')}
              </Button>
            ) : null}
            <Button
              type="text"
              size="small"
              disabled={
                !canManage ||
                row.is_default ||
                (row.kind === 'managed' && !row.published_revision)
              }
              onClick={() =>
                runAction(() =>
                  row.kind === 'official'
                    ? resetSettingsUiTemplateDefault(
                        {
                          provider_code: row.provider_code,
                          contribution_code: row.contribution_code
                        },
                        requireToken(csrfToken)
                      )
                    : setSettingsUiTemplateDefault(
                        row.id,
                        requireToken(csrfToken)
                      )
                )
              }
            >
              {t('set_default')}
            </Button>
            {row.kind === 'managed' ? (
              <Button
                type="text"
                size="small"
                danger
                disabled={!canManage || !csrfToken || actionPending}
                onClick={async () => {
                  await modal.confirm({
                    title: t('delete_template_title'),
                    content: t('delete_template_description', {
                      name: row.name
                    }),
                    okText: t('delete'),
                    cancelText: t('cancel'),
                    okButtonProps: { danger: true },
                    onOk: () =>
                      runActionAsync(() =>
                        deleteSettingsUiTemplate(
                          row.id,
                          requireToken(csrfToken)
                        )
                      )
                  });
                }}
              >
                {t('delete')}
              </Button>
            ) : null}
          </Space>
        )
      }
    ],
    [
      t,
      canManage,
      csrfToken,
      modal,
      runAction,
      runActionAsync,
      actionPending,
      openOfficial,
      openManaged
    ]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.ui_management.code_templates'
  });
  const [page, setPage] = useState(1);
  const pageSize = 10;
  const currentPage = Math.min(
    page,
    Math.max(1, Math.ceil(rows.length / pageSize))
  );

  const loadError = query.isError ? (
    <Alert
      type="error"
      showIcon
      title={t('templates_load_failed')}
      action={
        <Button loading={query.isFetching} onClick={() => void query.refetch()}>
          {t('routes_retry')}
        </Button>
      }
    />
  ) : undefined;

  return (
    <SettingsSectionSurface
      heightMode="fill"
      status={loadError}
      toolbar={
        <Flex justify="space-between" align="center" gap={8} wrap>
          <Button
            type="primary"
            disabled={!canManage || !query.data?.default_template}
            onClick={openCreate}
          >
            {t('new_template')}
          </Button>
          <DataTableColumnSettings
            columns={columns}
            configuration={tableConfiguration}
          />
        </Flex>
      }
    >
      {!query.isError || rows.length > 0 ? (
        <DataTable<TemplateRow>
          columns={columns}
          configuration={tableConfiguration}
          page={currentPage}
          pageSize={pageSize}
          total={rows.length}
          onPageChange={setPage}
          loading={query.isFetching}
          rowKey="key"
          onRow={(row) =>
            ({
              'data-template-id': row.kind === 'managed' ? row.id : undefined
            }) as HTMLAttributes<HTMLTableRowElement>
          }
          dataSource={rows.slice(
            (currentPage - 1) * pageSize,
            currentPage * pageSize
          )}
        />
      ) : null}
      {studio ? (
        <UiCodeTemplateStudio
          initialValue={studio.initialValue}
          managedTemplate={studio.managedTemplate}
          mode={studio.mode}
          officialTemplates={officialTemplates}
          open
          saving={save.isPending}
          workspaceId={workspaceId}
          onClose={() => setStudio(null)}
          onSave={(value) => save.mutate(value)}
        />
      ) : null}
    </SettingsSectionSurface>
  );
}
