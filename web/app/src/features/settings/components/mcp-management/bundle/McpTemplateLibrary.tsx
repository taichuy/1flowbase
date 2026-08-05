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
import { useCallback, useEffect, useState } from 'react';

import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  deleteSettingsMcpTemplateLibraryRelease,
  fetchSettingsMcpTemplateLibrary,
  refreshSettingsMcpTemplateLibrary,
  repairSettingsMcpTemplateLibraryRelease,
  setSettingsMcpTemplateLibraryCurrentVersion,
  settingsMcpCatalogQueryKey,
  settingsMcpTemplateLibraryQueryKey,
  syncSettingsMcpTemplateLibraryBundle,
  type SettingsMcpTemplateLibraryBundle,
  type SettingsMcpTemplateLibraryVersion
} from '../../../api/mcp-management';
import { settingsInstalledExtensionsQueryKey } from '../../../api/extensions';
import {
  McpBundleImportFlow,
  type McpBundleImportSource
} from './McpBundleImportFlow';
import './mcp-template-library.css';

interface VersionRow {
  bundle_version: string;
  remote: SettingsMcpTemplateLibraryVersion | null;
  local: SettingsMcpTemplateLibraryVersion | null;
}

function bundleKey(bundle: SettingsMcpTemplateLibraryBundle) {
  return `${bundle.organization}/${bundle.bundle_id}`;
}

function operationKey(
  bundle: SettingsMcpTemplateLibraryBundle,
  action: string,
  version?: string
) {
  return `${bundleKey(bundle)}:${version ?? 'latest'}:${action}`;
}

function latestRemote(bundle: SettingsMcpTemplateLibraryBundle) {
  return bundle.remote_versions[0] ?? null;
}

function versionRows(bundle: SettingsMcpTemplateLibraryBundle) {
  const rows = new Map<string, VersionRow>();
  for (const remote of bundle.remote_versions) {
    rows.set(remote.bundle_version, {
      bundle_version: remote.bundle_version,
      remote,
      local: null
    });
  }
  for (const local of bundle.local_versions) {
    const row = rows.get(local.bundle_version);
    rows.set(local.bundle_version, {
      bundle_version: local.bundle_version,
      remote: row?.remote ?? null,
      local
    });
  }
  return [...rows.values()];
}

export function McpTemplateLibrary({
  canManage = true,
  enabled = true,
  onImportOpen,
  variant = 'page'
}: {
  canManage?: boolean;
  enabled?: boolean;
  onImportOpen?: () => void;
  variant?: 'page' | 'compact';
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [importSource, setImportSource] =
    useState<McpBundleImportSource | null>(null);
  const [pending, setPending] = useState<Set<string>>(() => new Set());
  const libraryQuery = useQuery({
    queryKey: settingsMcpTemplateLibraryQueryKey,
    queryFn: fetchSettingsMcpTemplateLibrary,
    enabled: canManage && enabled,
    retry: false
  });
  const selected =
    libraryQuery.data?.bundles.find(
      (bundle) => bundleKey(bundle) === selectedKey
    ) ?? null;

  const refreshLibrary = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: settingsMcpTemplateLibraryQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: settingsInstalledExtensionsQueryKey(undefined)
      })
    ]);
  }, [queryClient]);
  const closeImportFlow = useCallback(() => setImportSource(null), []);
  const refreshMcpCatalog = useCallback(async () => {
    await queryClient.invalidateQueries({
      queryKey: settingsMcpCatalogQueryKey
    });
  }, [queryClient]);

  const start = (key: string) =>
    setPending((current) => new Set(current).add(key));
  const finish = (key: string) =>
    setPending((current) => {
      const next = new Set(current);
      next.delete(key);
      return next;
    });

  useEffect(() => {
    if (selectedKey && libraryQuery.data && !selected) setSelectedKey(null);
  }, [libraryQuery.data, selected, selectedKey]);

  async function syncBundle(
    bundle: SettingsMcpTemplateLibraryBundle,
    bundleVersion?: string
  ) {
    const key = operationKey(bundle, 'sync', bundleVersion);
    start(key);
    try {
      await syncSettingsMcpTemplateLibraryBundle(
        bundle.organization,
        bundle.bundle_id,
        bundleVersion ? { bundle_version: bundleVersion } : {},
        csrfToken
      );
      await refreshLibrary();
      messageApi.success(
        i18nText('settingsMcpManagement', 'auto.mcp_template_synced')
      );
      return true;
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
      return false;
    } finally {
      finish(key);
    }
  }

  async function checkRemoteCatalog() {
    const key = 'remote-catalog:refresh';
    start(key);
    try {
      const catalog = await refreshSettingsMcpTemplateLibrary();
      queryClient.setQueryData(settingsMcpTemplateLibraryQueryKey, catalog);
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
    } finally {
      finish(key);
    }
  }

  async function prepareImport(
    bundle: SettingsMcpTemplateLibraryBundle,
    bundleVersion?: string
  ) {
    const key = operationKey(bundle, 'prepare', bundleVersion);
    start(key);
    try {
      setImportSource({
        kind: 'library',
        organization: bundle.organization,
        bundleId: bundle.bundle_id,
        ...(bundleVersion ? { bundleVersion } : {})
      });
      onImportOpen?.();
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
    } finally {
      finish(key);
    }
  }

  async function setCurrent(
    bundle: SettingsMcpTemplateLibraryBundle,
    bundleVersion: string
  ) {
    const key = operationKey(bundle, 'current', bundleVersion);
    start(key);
    try {
      await setSettingsMcpTemplateLibraryCurrentVersion(
        bundle.organization,
        bundle.bundle_id,
        bundleVersion,
        csrfToken
      );
      await refreshLibrary();
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
    } finally {
      finish(key);
    }
  }

  async function repair(
    bundle: SettingsMcpTemplateLibraryBundle,
    bundleVersion: string
  ) {
    const key = operationKey(bundle, 'repair', bundleVersion);
    start(key);
    try {
      await repairSettingsMcpTemplateLibraryRelease(
        bundle.organization,
        bundle.bundle_id,
        bundleVersion,
        csrfToken
      );
      await refreshLibrary();
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
    } finally {
      finish(key);
    }
  }

  async function remove(
    bundle: SettingsMcpTemplateLibraryBundle,
    bundleVersion: string
  ) {
    const key = operationKey(bundle, 'delete', bundleVersion);
    start(key);
    try {
      await deleteSettingsMcpTemplateLibraryRelease(
        bundle.organization,
        bundle.bundle_id,
        bundleVersion,
        csrfToken
      );
      await refreshLibrary();
    } catch (error) {
      messageApi.error(error instanceof Error ? error.message : String(error));
    } finally {
      finish(key);
    }
  }

  const columns: TableProps<SettingsMcpTemplateLibraryBundle>['columns'] = [
    {
      title: i18nText('settingsMcpManagement', 'auto.mcp_bundle_name'),
      key: 'bundle',
      render: (_, bundle) => (
        <Typography.Text strong>{bundleKey(bundle)}</Typography.Text>
      )
    },
    {
      title: i18nText(
        'settingsMcpManagement',
        'auto.mcp_template_current_version'
      ),
      dataIndex: 'current_bundle_version',
      render: (value: string | null) => value ?? '—'
    },
    {
      title: i18nText(
        'settingsMcpManagement',
        'auto.mcp_template_remote_version'
      ),
      render: (_, bundle) => latestRemote(bundle)?.bundle_version ?? '—'
    },
    {
      title: i18nText('settingsMcpManagement', 'auto.mcp_bundle_action'),
      key: 'actions',
      render: (_, bundle) => {
        const latest = latestRemote(bundle);
        const syncKey = operationKey(bundle, 'sync');
        const canSync =
          libraryQuery.data?.remote_available === true &&
          latest !== null &&
          !bundle.local_versions.some(
            (version) => version.bundle_version === latest.bundle_version
          );
        return (
          <Space size={4} wrap>
            {bundle.current_bundle_version ? (
              <Button
                type="link"
                icon={<DownloadOutlined />}
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_bundle_import'
                )}
                loading={pending.has(operationKey(bundle, 'prepare'))}
                onClick={() => void prepareImport(bundle)}
              >
                {i18nText('settingsMcpManagement', 'auto.mcp_bundle_import')}
              </Button>
            ) : null}
            {canSync ? (
              <Button
                type="link"
                icon={<CloudDownloadOutlined />}
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_template_sync'
                )}
                loading={pending.has(syncKey)}
                onClick={() => void syncBundle(bundle)}
              >
                {i18nText('settingsMcpManagement', 'auto.mcp_template_sync')}
              </Button>
            ) : null}
            <Button
              type="link"
              icon={<EyeOutlined />}
              aria-label={i18nText(
                'settingsMcpManagement',
                'auto.mcp_template_view'
              )}
              onClick={() => setSelectedKey(bundleKey(bundle))}
            >
              {i18nText('settingsMcpManagement', 'auto.mcp_template_view')}
            </Button>
          </Space>
        );
      }
    }
  ];

  const versionColumns: TableProps<VersionRow>['columns'] = [
    {
      title: i18nText('settingsMcpManagement', 'auto.mcp_bundle_version'),
      dataIndex: 'bundle_version'
    },
    {
      title: i18nText(
        'settingsMcpManagement',
        'auto.mcp_template_availability'
      ),
      render: (_, row) => (
        <Space size={4} wrap>
          {row.local ? (
            <Tag>
              {i18nText('settingsMcpManagement', 'auto.mcp_template_local')}
            </Tag>
          ) : null}
          {row.remote ? (
            <Tag>
              {i18nText('settingsMcpManagement', 'auto.mcp_template_remote')}
            </Tag>
          ) : null}
          {selected?.current_bundle_version === row.bundle_version ? (
            <Tag color="processing">
              {i18nText('settingsMcpManagement', 'auto.mcp_template_current')}
            </Tag>
          ) : null}
        </Space>
      )
    },
    {
      title: i18nText(
        'settingsMcpManagement',
        'auto.mcp_bundle_source_version'
      ),
      render: (_, row) =>
        (row.local ?? row.remote)?.exported_from_system_version ?? '—'
    },
    {
      title: i18nText('settingsMcpManagement', 'auto.mcp_template_signature'),
      render: (_, row) => (row.local ?? row.remote)?.signature_status ?? '—'
    },
    {
      title: i18nText('settingsMcpManagement', 'auto.mcp_bundle_action'),
      render: (_, row) => {
        if (!selected) return null;
        const syncKey = operationKey(selected, 'sync', row.bundle_version);
        const currentKey = operationKey(
          selected,
          'current',
          row.bundle_version
        );
        const repairKey = operationKey(selected, 'repair', row.bundle_version);
        const deleteKey = operationKey(selected, 'delete', row.bundle_version);
        return (
          <Space size={4} wrap>
            {!row.local && row.remote ? (
              <Button
                type="link"
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_template_sync'
                )}
                loading={pending.has(syncKey)}
                onClick={() => void syncBundle(selected, row.bundle_version)}
              >
                {i18nText('settingsMcpManagement', 'auto.mcp_template_sync')}
              </Button>
            ) : null}
            {row.local ? (
              <>
                <Button
                  type="link"
                  aria-label={i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_template_import_version'
                  )}
                  loading={pending.has(
                    operationKey(selected, 'prepare', row.bundle_version)
                  )}
                  onClick={() =>
                    void prepareImport(selected, row.bundle_version)
                  }
                >
                  {i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_template_import_version'
                  )}
                </Button>
                {selected.current_bundle_version !== row.bundle_version ? (
                  <Button
                    type="link"
                    loading={pending.has(currentKey)}
                    onClick={() =>
                      void setCurrent(selected, row.bundle_version)
                    }
                  >
                    {i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_template_set_current'
                    )}
                  </Button>
                ) : null}
                <Button
                  type="link"
                  icon={<ToolOutlined />}
                  aria-label={i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_template_repair'
                  )}
                  loading={pending.has(repairKey)}
                  disabled={!row.remote}
                  onClick={() => void repair(selected, row.bundle_version)}
                >
                  {i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_template_repair'
                  )}
                </Button>
                <Popconfirm
                  title={i18nText(
                    'settingsMcpManagement',
                    'auto.mcp_template_delete_confirm'
                  )}
                  onConfirm={() => remove(selected, row.bundle_version)}
                >
                  <Button
                    type="link"
                    danger
                    icon={<DeleteOutlined />}
                    aria-label={i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_template_delete'
                    )}
                    loading={pending.has(deleteKey)}
                  >
                    {i18nText(
                      'settingsMcpManagement',
                      'auto.mcp_template_delete'
                    )}
                  </Button>
                </Popconfirm>
              </>
            ) : null}
          </Space>
        );
      }
    }
  ];

  if (!canManage) return null;

  return (
    <div className={`mcp-template-library mcp-template-library--${variant}`}>
      {messageContextHolder}
      <Flex justify="space-between" align="center" gap={12} wrap>
        <Typography.Text type="secondary">
          {i18nText(
            'settingsMcpManagement',
            'auto.mcp_template_library_description'
          )}
        </Typography.Text>
        <Button
          icon={<ReloadOutlined />}
          aria-label={i18nText(
            'settingsMcpManagement',
            'auto.mcp_template_refresh'
          )}
          loading={pending.has('remote-catalog:refresh')}
          onClick={() => void checkRemoteCatalog()}
        >
          {i18nText('settingsMcpManagement', 'auto.mcp_template_refresh')}
        </Button>
      </Flex>
      {libraryQuery.data?.remote_error ? (
        <Alert
          showIcon
          type="warning"
          title={i18nText(
            'settingsMcpManagement',
            'auto.mcp_template_remote_unavailable'
          )}
        />
      ) : null}
      {libraryQuery.isError ? (
        <Alert
          showIcon
          type="error"
          title={
            libraryQuery.error instanceof Error
              ? libraryQuery.error.message
              : String(libraryQuery.error)
          }
        />
      ) : null}
      <Table
        rowKey={bundleKey}
        columns={columns}
        dataSource={libraryQuery.data?.bundles ?? []}
        loading={libraryQuery.isLoading}
        pagination={false}
        scroll={{ x: 800 }}
      />
      <Drawer
        open={Boolean(selected)}
        title={selected ? bundleKey(selected) : undefined}
        size={variant === 'compact' ? 760 : 900}
        onClose={() => setSelectedKey(null)}
      >
        {selected ? (
          <Flex vertical gap={16}>
            <Descriptions bordered size="small" column={1}>
              <Descriptions.Item
                label={i18nText(
                  'settingsMcpManagement',
                  'auto.mcp_template_current_version'
                )}
              >
                {selected.current_bundle_version ?? '—'}
              </Descriptions.Item>
            </Descriptions>
            <Table
              rowKey="bundle_version"
              columns={versionColumns}
              dataSource={versionRows(selected)}
              pagination={false}
              scroll={{ x: 900 }}
            />
          </Flex>
        ) : null}
      </Drawer>
      <McpBundleImportFlow
        source={importSource}
        csrfToken={csrfToken}
        onClose={closeImportFlow}
        onApplied={refreshMcpCatalog}
      />
    </div>
  );
}
