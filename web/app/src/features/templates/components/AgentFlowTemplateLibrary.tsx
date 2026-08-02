import {
  CloudDownloadOutlined,
  DeleteOutlined,
  DownloadOutlined,
  EyeOutlined,
  ReloadOutlined,
  ToolOutlined
} from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Drawer,
  Flex,
  Popconfirm,
  Space,
  Table,
  Tag,
  Typography,
  message,
  type TableProps
} from 'antd';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { applicationsQueryKey } from '../../applications/api/applications';
import { ApplicationTemplateImportModal } from '../../applications/components/ApplicationTemplateImportModal';
import { formatDateTime } from '../../../shared/i18n/format';
import { useAuthStore } from '../../../state/auth-store';
import {
  deleteOfficialAgentFlowTemplateRelease,
  fetchOfficialAgentFlowTemplateCatalog,
  importOfficialAgentFlowTemplate,
  officialAgentFlowTemplateCatalogQueryKey,
  previewOfficialAgentFlowTemplate,
  repairOfficialAgentFlowTemplateRelease,
  switchOfficialAgentFlowTemplateCurrent,
  syncOfficialAgentFlowTemplate,
  type AgentFlowTemplateLibraryEntry,
  type AgentFlowTemplatePreview
} from '../api/templates';
import './agent-flow-template-library.css';

type LibraryVariant = 'page' | 'compact';

interface PreparedImport {
  template_id: string;
  release_version: number | undefined;
  preview: AgentFlowTemplatePreview;
}

interface VersionRow {
  release_version: number;
  remote: AgentFlowTemplateLibraryEntry['remote_versions'][number] | null;
  local: AgentFlowTemplateLibraryEntry['local_versions'][number] | null;
}

function latestRemoteVersion(entry: AgentFlowTemplateLibraryEntry) {
  return entry.remote_versions.reduce<number | null>(
    (latest, version) =>
      latest === null || version.release_version > latest
        ? version.release_version
        : latest,
    null
  );
}

function templateApplication(entry: AgentFlowTemplateLibraryEntry) {
  return (
    entry.local_versions.find(
      (version) => version.release_version === entry.current_release_version
    )?.application ??
    entry.remote_versions.find(
      (version) => version.release_version === latestRemoteVersion(entry)
    )?.application ??
    entry.local_versions[0]?.application ??
    entry.remote_versions[0]?.application
  );
}

function versionRows(entry: AgentFlowTemplateLibraryEntry): VersionRow[] {
  const rows = new Map<number, VersionRow>();
  for (const remote of entry.remote_versions) {
    rows.set(remote.release_version, {
      release_version: remote.release_version,
      remote,
      local: null
    });
  }
  for (const local of entry.local_versions) {
    const row = rows.get(local.release_version);
    rows.set(local.release_version, {
      release_version: local.release_version,
      remote: row?.remote ?? null,
      local
    });
  }
  return [...rows.values()].sort(
    (left, right) => right.release_version - left.release_version
  );
}

function operationKey(
  templateId: string,
  action: string,
  releaseVersion?: number
) {
  return `${templateId}:${releaseVersion ?? 'latest'}:${action}`;
}

export function AgentFlowTemplateLibrary({
  variant = 'page'
}: {
  variant?: LibraryVariant;
}) {
  const { t } = useTranslation('templates');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [selected, setSelected] =
    useState<AgentFlowTemplateLibraryEntry | null>(null);
  const [preparedImport, setPreparedImport] = useState<PreparedImport | null>(
    null
  );
  const [importName, setImportName] = useState('');
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  const catalogQuery = useQuery({
    queryKey: officialAgentFlowTemplateCatalogQueryKey,
    queryFn: fetchOfficialAgentFlowTemplateCatalog,
    retry: false
  });

  useEffect(() => {
    if (!selected || !catalogQuery.data) return;
    setSelected(
      catalogQuery.data.templates.find(
        (entry) => entry.template_id === selected.template_id
      ) ?? null
    );
  }, [catalogQuery.data, selected]);

  async function refreshCatalog() {
    await queryClient.invalidateQueries({
      queryKey: officialAgentFlowTemplateCatalogQueryKey
    });
  }

  async function prepareImport(
    entry: AgentFlowTemplateLibraryEntry,
    releaseVersion: number | undefined
  ) {
    const key = operationKey(entry.template_id, 'preview', releaseVersion);
    setPendingKey(key);
    try {
      const preview = await previewOfficialAgentFlowTemplate(
        entry.template_id,
        releaseVersion,
        csrfToken
      );
      setPreparedImport({
        template_id: entry.template_id,
        release_version: releaseVersion,
        preview
      });
      setImportName(preview.application.name);
      if (entry.local_versions.length === 0) await refreshCatalog();
    } catch {
      messageApi.error(t('auto.template_prepare_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  async function importTemplate() {
    if (!preparedImport) return;
    const key = operationKey(
      preparedImport.template_id,
      'import',
      preparedImport.release_version
    );
    setPendingKey(key);
    try {
      const imported = await importOfficialAgentFlowTemplate(
        preparedImport.template_id,
        {
          ...(preparedImport.release_version === undefined
            ? {}
            : { release_version: preparedImport.release_version }),
          name: importName.trim(),
          description: preparedImport.preview.application.description
        },
        csrfToken
      );
      await queryClient.invalidateQueries({ queryKey: applicationsQueryKey });
      messageApi.success(t('auto.template_imported'));
      setPreparedImport(null);
      window.location.assign(
        `/applications/${imported.application.id}/orchestration`
      );
    } catch {
      messageApi.error(t('auto.template_import_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  async function syncTemplate(
    entry: AgentFlowTemplateLibraryEntry,
    releaseVersion: number | undefined
  ) {
    const key = operationKey(entry.template_id, 'sync', releaseVersion);
    setPendingKey(key);
    try {
      await syncOfficialAgentFlowTemplate(
        entry.template_id,
        releaseVersion,
        csrfToken
      );
      await refreshCatalog();
      messageApi.success(t('auto.template_synced'));
    } catch {
      messageApi.error(t('auto.template_sync_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  async function switchCurrent(
    entry: AgentFlowTemplateLibraryEntry,
    releaseVersion: number
  ) {
    const key = operationKey(entry.template_id, 'current', releaseVersion);
    setPendingKey(key);
    try {
      await switchOfficialAgentFlowTemplateCurrent(
        entry.template_id,
        releaseVersion,
        csrfToken
      );
      await refreshCatalog();
      messageApi.success(t('auto.current_version_changed'));
    } catch {
      messageApi.error(t('auto.current_version_change_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  async function repairRelease(
    entry: AgentFlowTemplateLibraryEntry,
    releaseVersion: number
  ) {
    const key = operationKey(entry.template_id, 'repair', releaseVersion);
    setPendingKey(key);
    try {
      await repairOfficialAgentFlowTemplateRelease(
        entry.template_id,
        releaseVersion,
        csrfToken
      );
      await refreshCatalog();
      messageApi.success(t('auto.template_repaired'));
    } catch {
      messageApi.error(t('auto.template_repair_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  async function deleteRelease(
    entry: AgentFlowTemplateLibraryEntry,
    releaseVersion: number
  ) {
    const key = operationKey(entry.template_id, 'delete', releaseVersion);
    setPendingKey(key);
    try {
      await deleteOfficialAgentFlowTemplateRelease(
        entry.template_id,
        releaseVersion,
        csrfToken
      );
      await refreshCatalog();
      messageApi.success(t('auto.template_release_deleted'));
    } catch {
      messageApi.error(t('auto.template_release_delete_failed'));
    } finally {
      setPendingKey((current) => (current === key ? null : current));
    }
  }

  const columns: TableProps<AgentFlowTemplateLibraryEntry>['columns'] = [
    {
      title: t('auto.template_info'),
      key: 'template',
      width: 280,
      render: (_, entry) => {
        const application = templateApplication(entry);
        return (
          <Space direction="vertical" size={0}>
            <Typography.Text strong>
              {application?.name ?? entry.template_id}
            </Typography.Text>
            <Typography.Text type="secondary" copyable>
              {entry.template_id}
            </Typography.Text>
          </Space>
        );
      }
    },
    {
      title: t('auto.description'),
      key: 'description',
      width: 300,
      render: (_, entry) =>
        templateApplication(entry)?.description || t('auto.description_empty')
    },
    {
      title: t('auto.current_version'),
      dataIndex: 'current_release_version',
      key: 'current_release_version',
      width: 140,
      render: (version: number | null) =>
        version === null ? t('auto.not_imported_locally') : `v${version}`
    },
    {
      title: t('auto.latest_remote_version'),
      key: 'latest_remote_version',
      width: 150,
      render: (_, entry) => {
        const version = latestRemoteVersion(entry);
        return version === null ? '—' : `v${version}`;
      }
    },
    {
      title: t('auto.actions'),
      key: 'actions',
      width: 260,
      fixed: 'right',
      render: (_, entry) => {
        const currentVersion = entry.current_release_version ?? undefined;
        const previewKey = operationKey(
          entry.template_id,
          'preview',
          currentVersion
        );
        const syncKey = operationKey(entry.template_id, 'sync');
        const latest = latestRemoteVersion(entry);
        const canSync =
          catalogQuery.data?.remote_available === true &&
          latest !== null &&
          (entry.current_release_version === null ||
            latest > entry.current_release_version);
        return (
          <Space size={4} wrap>
            <Button
              type="link"
              icon={<DownloadOutlined />}
              loading={pendingKey === previewKey}
              disabled={pendingKey !== null && pendingKey !== previewKey}
              aria-label={`${t('auto.import_template')}-${templateApplication(entry)?.name ?? entry.template_id}`}
              onClick={() => void prepareImport(entry, currentVersion)}
            >
              {t('auto.import_template')}
            </Button>
            {canSync ? (
              <Button
                type="link"
                icon={<CloudDownloadOutlined />}
                loading={pendingKey === syncKey}
                disabled={pendingKey !== null && pendingKey !== syncKey}
                onClick={() => void syncTemplate(entry, undefined)}
              >
                {t('auto.sync')}
              </Button>
            ) : null}
            <Button
              type="link"
              icon={<EyeOutlined />}
              onClick={() => setSelected(entry)}
            >
              {t('auto.view')}
            </Button>
          </Space>
        );
      }
    }
  ];

  const selectedRows = selected ? versionRows(selected) : [];
  const versionColumns: TableProps<VersionRow>['columns'] = [
    {
      title: t('auto.release_version'),
      dataIndex: 'release_version',
      key: 'release_version',
      width: 110,
      render: (version: number) => `v${version}`
    },
    {
      title: t('auto.availability'),
      key: 'availability',
      width: 170,
      render: (_, row) => (
        <Space size={4} wrap>
          {row.local ? <Tag>{t('auto.local')}</Tag> : null}
          {row.remote ? <Tag>{t('auto.remote')}</Tag> : null}
          {selected?.current_release_version === row.release_version ? (
            <Tag color="processing">{t('auto.current')}</Tag>
          ) : null}
        </Space>
      )
    },
    {
      title: t('auto.exported_at'),
      key: 'exported_at',
      width: 180,
      render: (_, row) =>
        formatDateTime((row.local ?? row.remote)!.exported_at, {
          hour12: false
        })
    },
    {
      title: t('auto.checksum'),
      key: 'checksum',
      width: 220,
      render: (_, row) => (
        <Typography.Text copyable ellipsis>
          {(row.local ?? row.remote)!.checksum}
        </Typography.Text>
      )
    },
    {
      title: t('auto.actions'),
      key: 'actions',
      width: 360,
      fixed: 'right',
      render: (_, row) => {
        if (!selected) return null;
        const previewKey = operationKey(
          selected.template_id,
          'preview',
          row.release_version
        );
        const syncKey = operationKey(
          selected.template_id,
          'sync',
          row.release_version
        );
        const currentKey = operationKey(
          selected.template_id,
          'current',
          row.release_version
        );
        const repairKey = operationKey(
          selected.template_id,
          'repair',
          row.release_version
        );
        const deleteKey = operationKey(
          selected.template_id,
          'delete',
          row.release_version
        );
        const disabled = (key: string) =>
          pendingKey !== null && pendingKey !== key;
        return (
          <Space size={4} wrap>
            {!row.local && row.remote ? (
              <Button
                type="link"
                loading={pendingKey === syncKey}
                disabled={
                  disabled(syncKey) || !catalogQuery.data?.remote_available
                }
                onClick={() => void syncTemplate(selected, row.release_version)}
              >
                {t('auto.sync_to_local')}
              </Button>
            ) : null}
            {row.local ? (
              <>
                <Button
                  type="link"
                  loading={pendingKey === previewKey}
                  disabled={disabled(previewKey)}
                  onClick={() =>
                    void prepareImport(selected, row.release_version)
                  }
                >
                  {t('auto.import_this_version')}
                </Button>
                {selected.current_release_version !== row.release_version ? (
                  <Button
                    type="link"
                    loading={pendingKey === currentKey}
                    disabled={disabled(currentKey)}
                    onClick={() =>
                      void switchCurrent(selected, row.release_version)
                    }
                  >
                    {t('auto.set_current')}
                  </Button>
                ) : null}
                <Button
                  type="link"
                  icon={<ToolOutlined />}
                  loading={pendingKey === repairKey}
                  disabled={
                    disabled(repairKey) ||
                    !catalogQuery.data?.remote_available ||
                    !row.remote
                  }
                  onClick={() =>
                    void repairRelease(selected, row.release_version)
                  }
                >
                  {t('auto.repair')}
                </Button>
                <Popconfirm
                  title={t('auto.confirm_delete_release')}
                  okText={t('auto.delete')}
                  cancelText={t('auto.cancel')}
                  onConfirm={() => deleteRelease(selected, row.release_version)}
                >
                  <Button
                    type="link"
                    danger
                    icon={<DeleteOutlined />}
                    loading={pendingKey === deleteKey}
                    disabled={disabled(deleteKey)}
                  >
                    {t('auto.delete')}
                  </Button>
                </Popconfirm>
              </>
            ) : null}
          </Space>
        );
      }
    }
  ];

  return (
    <div
      className={`agent-flow-template-library agent-flow-template-library--${variant}`}
    >
      {messageContextHolder}
      <Flex justify="space-between" align="center" gap={12} wrap>
        <Typography.Text type="secondary">
          {t('auto.local_template_library_description')}
        </Typography.Text>
        <Button
          icon={<ReloadOutlined />}
          loading={catalogQuery.isFetching}
          onClick={() => void catalogQuery.refetch()}
        >
          {t('auto.refresh_catalog')}
        </Button>
      </Flex>

      {catalogQuery.data && !catalogQuery.data.remote_available ? (
        <Alert
          type="warning"
          showIcon
          message={t('auto.remote_unavailable')}
          description={t('auto.remote_unavailable_local_available')}
        />
      ) : null}
      {catalogQuery.isError ? (
        <Alert
          type="error"
          showIcon
          message={t('auto.catalog_load_failed')}
          action={
            <Button size="small" onClick={() => void catalogQuery.refetch()}>
              {t('auto.retry')}
            </Button>
          }
        />
      ) : null}

      <Table
        rowKey="template_id"
        columns={columns}
        dataSource={catalogQuery.data?.templates ?? []}
        loading={catalogQuery.isLoading}
        pagination={false}
        scroll={{ x: 1130 }}
        locale={{ emptyText: t('auto.empty_catalog') }}
      />

      <Drawer
        open={Boolean(selected)}
        title={
          selected
            ? (templateApplication(selected)?.name ?? selected.template_id)
            : undefined
        }
        width={variant === 'compact' ? 760 : 900}
        onClose={() => setSelected(null)}
      >
        {selected ? (
          <Flex vertical gap={16}>
            <Descriptions column={1} bordered size="small">
              <Descriptions.Item label={t('auto.template_id')}>
                <Typography.Text copyable>
                  {selected.template_id}
                </Typography.Text>
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.current_version')}>
                {selected.current_release_version === null
                  ? t('auto.not_imported_locally')
                  : `v${selected.current_release_version}`}
              </Descriptions.Item>
            </Descriptions>
            <Table
              rowKey="release_version"
              size="small"
              columns={versionColumns}
              dataSource={selectedRows}
              pagination={false}
              scroll={{ x: 1040 }}
            />
          </Flex>
        ) : null}
      </Drawer>

      <ApplicationTemplateImportModal
        open={Boolean(preparedImport)}
        preview={preparedImport?.preview ?? null}
        name={importName}
        importing={
          preparedImport
            ? pendingKey ===
              operationKey(
                preparedImport.template_id,
                'import',
                preparedImport.release_version
              )
            : false
        }
        onNameChange={setImportName}
        onCancel={() => setPreparedImport(null)}
        onImport={() => void importTemplate()}
      />
    </div>
  );
}
