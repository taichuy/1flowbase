import { useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App, Button, Space, Table, Tag } from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  archiveSettingsUiTemplate,
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
  initialValue: SettingsUiTemplateInput;
};

type OfficialRow = SettingsUiOfficialTemplate & {
  key: string;
  kind: 'official';
  name: string;
  revision: string;
  status: 'published';
  is_archived: false;
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
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const queryClient = useQueryClient();
  const [studio, setStudio] = useState<StudioSession | null>(null);
  const [includeArchived, setIncludeArchived] = useState(false);
  const query = useQuery({
    queryKey: [...settingsUiTemplatesQueryKey, includeArchived],
    queryFn: () => fetchSettingsUiTemplates(includeArchived)
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
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: settingsUiTemplatesQueryKey }),
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
        status: 'published' as const,
        is_archived: false as const
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

  const openCreate = () =>
    setStudio({
      mode: 'create',
      templateId: null,
      initialValue: {
        provider_code: '',
        contribution_code: '',
        name: '',
        source: '',
        language: 'tsx'
      }
    });
  const openOfficial = (row: OfficialRow, mode: 'view' | 'copy') =>
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
    });
  const openManaged = (row: ManagedRow, mode: 'edit' | 'copy') =>
    setStudio({
      mode,
      templateId: mode === 'edit' ? row.id : null,
      initialValue: {
        provider_code: row.provider_code,
        contribution_code: row.contribution_code,
        name:
          mode === 'copy' ? `${row.name} - ${t('copy_name_suffix')}` : row.name,
        source: row.latest_revision.source,
        language: row.latest_revision.language
      }
    });

  return (
    <SettingsSectionSurface
      toolbar={
        <Space wrap>
          <Button type="primary" disabled={!canManage} onClick={openCreate}>
            {t('new_template')}
          </Button>
          <Button onClick={() => setIncludeArchived((value) => !value)}>
            {includeArchived ? t('hide_archived') : t('show_archived')}
          </Button>
        </Space>
      }
    >
      <Table<TemplateRow>
        loading={query.isLoading}
        rowKey="key"
        scroll={{ x: 980 }}
        dataSource={rows}
        columns={[
          { title: t('name'), dataIndex: 'name' },
          {
            title: t('contribution'),
            render: (_, row) => `${row.provider_code}/${row.contribution_code}`
          },
          {
            title: t('source'),
            render: (_, row) => (
              <Tag>
                {row.kind === 'official' ? t('official') : t('managed')}
              </Tag>
            )
          },
          { title: t('revision'), dataIndex: 'revision' },
          {
            title: t('status'),
            render: (_, row) => (
              <Space>
                <Tag color={row.status === 'published' ? 'green' : 'default'}>
                  {t(row.status)}
                </Tag>
                {row.is_default ? <Tag color="blue">{t('default')}</Tag> : null}
                {row.is_archived ? <Tag>{t('archived')}</Tag> : null}
              </Space>
            )
          },
          {
            title: t('actions'),
            fixed: 'right',
            render: (_, row) => (
              <Space wrap>
                {row.kind === 'official' ? (
                  <Button
                    size="small"
                    onClick={() => openOfficial(row, 'view')}
                  >
                    {t('view')}
                  </Button>
                ) : (
                  <Button
                    size="small"
                    disabled={!canManage || row.is_archived}
                    onClick={() => openManaged(row, 'edit')}
                  >
                    {t('edit')}
                  </Button>
                )}
                <Button
                  size="small"
                  disabled={!canManage || row.is_archived}
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
                    size="small"
                    disabled={
                      !canManage ||
                      row.latest_revision.is_published ||
                      row.is_archived
                    }
                    onClick={() =>
                      action.mutate(() =>
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
                  size="small"
                  disabled={
                    !canManage ||
                    row.is_default ||
                    row.is_archived ||
                    (row.kind === 'managed' && !row.published_revision)
                  }
                  onClick={() =>
                    action.mutate(() =>
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
                    size="small"
                    danger={!row.is_archived}
                    disabled={!canManage}
                    onClick={() =>
                      action.mutate(() =>
                        archiveSettingsUiTemplate(
                          row.id,
                          !row.is_archived,
                          requireToken(csrfToken)
                        )
                      )
                    }
                  >
                    {row.is_archived ? t('restore') : t('archive')}
                  </Button>
                ) : null}
              </Space>
            )
          }
        ]}
      />
      {studio ? (
        <UiCodeTemplateStudio
          initialValue={studio.initialValue}
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
