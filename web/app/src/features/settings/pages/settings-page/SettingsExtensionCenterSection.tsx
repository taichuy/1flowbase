import { useCallback, useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Badge,
  Button,
  Descriptions,
  Drawer,
  Empty,
  Flex,
  List,
  Modal,
  Space,
  Tabs,
  Tag,
  Tooltip,
  Typography,
  message
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  applySettingsInstalledMcpExtension,
  checkSettingsExtensionUpdates,
  fetchSettingsExtensionCatalog,
  fetchSettingsExtensionCatalogEntry,
  fetchSettingsInstalledExtensions,
  getSettingsInstalledMcpExtensionConflict,
  getSettingsExtensionRiskChallenge,
  installSettingsExtension,
  previewSettingsInstalledMcpExtension,
  settingsExtensionCatalogQueryKey,
  settingsInstalledExtensionsQueryKey,
  type SettingsExtensionCatalogEntry,
  type SettingsExtensionCategory,
  type SettingsExtensionCenterCategory,
  type SettingsInstalledExtension
} from '../../api/extensions';
import { settingsMcpCatalogQueryKey } from '../../api/mcp-management';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

type ExtensionRow = SettingsInstalledExtension | SettingsExtensionCatalogEntry;
type UpdateState =
  | 'checking'
  | 'current'
  | 'update_available'
  | 'unknown_error';
type ExtensionOperation = {
  kind: 'catalog';
  entry: SettingsExtensionCatalogEntry;
  update: boolean;
};
type ExtensionOverrides = Parameters<typeof installSettingsExtension>[2];

const CATEGORIES: SettingsExtensionCategory[] = [
  'agent-flow',
  'capability-plugins',
  'host-extensions',
  'i18n',
  'mcp',
  'runtime-extensions'
];

function isInstalledRow(row: ExtensionRow): row is SettingsInstalledExtension {
  return 'node_id' in row;
}

function extensionCatalogId(row: ExtensionRow) {
  return isInstalledRow(row) ? row.catalog_id : row.id;
}

function extensionName(row: ExtensionRow) {
  return isInstalledRow(row) ? row.artifact_id : row.name;
}

function extensionVersion(row: ExtensionRow) {
  return row.version;
}

function extensionHostRequirement(row: ExtensionRow) {
  return isInstalledRow(row) ? null : row.host_version_requirement;
}

function extensionSource(row: ExtensionRow) {
  return isInstalledRow(row) ? row.source : row.catalog_source;
}

function extensionDescription(row: ExtensionRow) {
  return isInstalledRow(row) ? null : row.description;
}

function extensionInstallationStatus(row: ExtensionRow) {
  return isInstalledRow(row) ? row.status : row.installation_status;
}

function extensionKey(row: ExtensionRow) {
  return extensionCatalogId(row);
}

export function SettingsExtensionCenterSection({
  category: activeTab,
  cursor
}: {
  category: SettingsExtensionCenterCategory;
  cursor?: string;
}) {
  const { t } = useTranslation('settings');
  const navigate = useNavigate();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<ExtensionRow | null>(null);
  const [updateStates, setUpdateStates] = useState<Record<string, UpdateState>>(
    {}
  );
  const [resolvingUpdateKey, setResolvingUpdateKey] = useState<string | null>(
    null
  );

  useEffect(() => {
    setSelected(null);
    setUpdateStates({});
  }, [activeTab, cursor]);

  const installedQuery = useQuery({
    queryKey: settingsInstalledExtensionsQueryKey(cursor),
    queryFn: () => fetchSettingsInstalledExtensions(cursor),
    enabled: activeTab === 'installed',
    retry: false
  });
  const catalogQuery = useQuery({
    queryKey:
      activeTab === 'installed'
        ? ['settings', 'extension-center', 'catalog', 'inactive']
        : settingsExtensionCatalogQueryKey(activeTab, cursor),
    queryFn: async () => {
      const category = activeTab;
      if (category === 'installed') throw new Error('catalog tab required');
      const page = await fetchSettingsExtensionCatalog(category, cursor);
      if (
        page.category !== category ||
        page.entries.some((entry) => entry.category !== category)
      ) {
        throw new Error('extension catalog category mismatch');
      }
      return page;
    },
    enabled: activeTab !== 'installed',
    retry: false
  });

  const rows: ExtensionRow[] = useMemo(
    () =>
      activeTab === 'installed'
        ? (installedQuery.data?.entries ?? [])
        : catalogQuery.data?.category === activeTab
          ? catalogQuery.data.entries
          : [],
    [activeTab, catalogQuery.data?.entries, installedQuery.data?.entries]
  );

  useEffect(() => {
    if (!csrfToken || rows.length === 0) return;

    const checkableRows = rows.filter(
      (row) => isInstalledRow(row) || row.current_version !== null
    );
    const groups = new Map<SettingsExtensionCategory, ExtensionRow[]>();
    for (const row of checkableRows) {
      const group = groups.get(row.category) ?? [];
      group.push(row);
      groups.set(row.category, group);
    }
    if (groups.size === 0) return;

    const checking = Object.fromEntries(
      checkableRows.map((row) => [extensionKey(row), 'checking' as const])
    );
    setUpdateStates((current) => ({ ...current, ...checking }));

    let cancelled = false;
    void Promise.all(
      [...groups.entries()].map(async ([category, entries]) => {
        try {
          const result = await checkSettingsExtensionUpdates(
            {
              category,
              catalog_page:
                activeTab === 'installed'
                  ? null
                  : (catalogQuery.data?.catalog_page ?? null),
              items: entries.map((entry) => ({
                catalog_id: extensionCatalogId(entry),
                current_version: isInstalledRow(entry)
                  ? entry.version
                  : entry.current_version!,
                installed_versions: isInstalledRow(entry)
                  ? entry.installed_versions.map((version) => version.version)
                  : [entry.current_version!]
              }))
            },
            csrfToken
          );
          return result.items.map(
            (item) => [item.catalog_id, item.status] as const
          );
        } catch {
          return entries.map(
            (entry) => [extensionKey(entry), 'unknown_error'] as const
          );
        }
      })
    ).then((results) => {
      if (!cancelled) {
        setUpdateStates((current) => ({
          ...current,
          ...Object.fromEntries(results.flat())
        }));
      }
    });

    return () => {
      cancelled = true;
    };
  }, [activeTab, catalogQuery.data?.catalog_page, csrfToken, cursor, rows]);

  const invalidateExtensionAndMcpState = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      }),
      queryClient.invalidateQueries({ queryKey: settingsMcpCatalogQueryKey })
    ]);
  }, [queryClient]);

  const applyInstalledMcpExtension = useCallback(
    async (
      extensionInstallationId: string,
      conflictResolution?: 'keep_existing'
    ) => {
      if (!csrfToken) throw new Error('csrf token required');
      try {
        await applySettingsInstalledMcpExtension(
          extensionInstallationId,
          csrfToken,
          conflictResolution
        );
        message.success(t('auto.mcp_extension_apply_succeeded'));
        await invalidateExtensionAndMcpState();
      } catch (error) {
        const conflict = getSettingsInstalledMcpExtensionConflict(error);
        if (!conflict) {
          message.error(t('auto.mcp_extension_apply_failed'));
          return;
        }
        Modal.confirm({
          title: t('auto.mcp_extension_conflict_title'),
          content: t('auto.mcp_extension_conflict_keep_existing'),
          okText: t('auto.confirm'),
          cancelText: t('auto.cancel'),
          onOk: () =>
            applyInstalledMcpExtension(
              conflict.extension_installation_id,
              conflict.required_conflict_resolution
            )
        });
      }
    },
    [csrfToken, invalidateExtensionAndMcpState, t]
  );

  const previewInstalledMcpExtension = useCallback(
    async (extensionInstallationId: string) => {
      if (!csrfToken) throw new Error('csrf token required');
      try {
        const result = await previewSettingsInstalledMcpExtension(
          extensionInstallationId,
          csrfToken
        );
        const conflictResolution = result.required_conflict_resolution;
        Modal.confirm({
          title: conflictResolution
            ? t('auto.mcp_extension_conflict_title')
            : t('auto.mcp_extension_apply_title'),
          content: conflictResolution
            ? t('auto.mcp_extension_conflict_keep_existing')
            : t('auto.mcp_extension_apply_confirmation'),
          okText: t('auto.confirm'),
          cancelText: t('auto.cancel'),
          onOk: () =>
            applyInstalledMcpExtension(
              result.extension_installation_id,
              conflictResolution ?? undefined
            )
        });
      } catch {
        message.error(t('auto.mcp_extension_preview_failed'));
      }
    },
    [applyInstalledMcpExtension, csrfToken, t]
  );

  const operationMutation = useMutation({
    mutationFn: async ({
      operation,
      overrides = {}
    }: {
      operation: ExtensionOperation;
      overrides?: ExtensionOverrides;
    }) => {
      if (!csrfToken) throw new Error('csrf token required');
      try {
        const result = await installSettingsExtension(
          operation.entry,
          csrfToken,
          overrides,
          operation.update
        );
        return { challenge: null, operation, result };
      } catch (error) {
        const challenge = getSettingsExtensionRiskChallenge(error);
        if (!challenge) throw error;
        return { challenge, operation, result: null };
      }
    },
    onSuccess: async ({ challenge, operation, result }) => {
      if (challenge) {
        const acknowledgedWarnings = challenge.warnings
          .filter((warning) => warning.overridable)
          .map((warning) => warning.code);
        Modal.confirm({
          title: t('auto.risk_warnings'),
          content: (
            <List
              size="small"
              dataSource={challenge.warnings}
              renderItem={(warning) => <List.Item>{warning.message}</List.Item>}
            />
          ),
          okText: t('auto.confirm'),
          cancelText: t('auto.cancel'),
          onOk: () =>
            operationMutation.mutateAsync({
              operation,
              overrides: {
                ...(acknowledgedWarnings.length > 0
                  ? {
                      risk_override: {
                        reason: 'user_confirmed',
                        acknowledged_warnings: acknowledgedWarnings
                      }
                    }
                  : {}),
                ...(challenge.compatibility
                  ? {
                      compatibility_override: {
                        reason: challenge.compatibility.reason,
                        acknowledged_current_host_version:
                          challenge.compatibility.current_host_version,
                        acknowledged_minimum_host_version:
                          challenge.compatibility.minimum_host_version
                      }
                    }
                  : {})
              }
            })
        });
        return;
      }

      message.success(t('auto.extension_operation_submitted'));
      await queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      });
      if (
        operation.entry.category === 'mcp' &&
        result?.workspace_application_status === 'not_imported'
      ) {
        await previewInstalledMcpExtension(result.installation.id);
      }
    },
    onError: () => message.error(t('auto.extension_operation_failed'))
  });
  const runOperation = operationMutation.mutateAsync;

  const submitOperation = useCallback(
    (operation: ExtensionOperation) => {
      Modal.confirm({
        title: operation.update
          ? t('auto.update_extension')
          : t('auto.install_extension'),
        content: t('auto.extension_install_confirmation'),
        okText: t('auto.confirm'),
        cancelText: t('auto.cancel'),
        onOk: () => runOperation({ operation })
      });
    },
    [runOperation, t]
  );

  const resolveInstalledUpdate = useCallback(
    async (row: SettingsInstalledExtension) => {
      const key = extensionKey(row);
      setResolvingUpdateKey(key);
      try {
        const entry = await fetchSettingsExtensionCatalogEntry(
          row.category,
          row.catalog_id
        );
        submitOperation({ kind: 'catalog', entry, update: true });
      } catch {
        setUpdateStates((current) => ({
          ...current,
          [key]: 'unknown_error'
        }));
        message.error(t('auto.extension_operation_failed'));
      } finally {
        setResolvingUpdateKey(null);
      }
    },
    [submitOperation, t]
  );

  const columns = useMemo<Array<DataTableColumn<ExtensionRow>>>(
    () => [
      {
        title: t('auto.name'),
        key: 'name',
        width: 180,
        render: (_, row) => extensionName(row),
        ellipsis: true
      },
      {
        title: t('auto.kind'),
        dataIndex: 'category',
        key: 'category',
        width: 180,
        render: (value) => <Tag>{String(value)}</Tag>
      },
      {
        title: t('auto.description'),
        key: 'description',
        width: 280,
        sizing: 'fill',
        render: (_, row) => extensionDescription(row) ?? '—',
        ellipsis: true
      },
      {
        title: t('auto.current_version'),
        key: 'version',
        width: 130,
        render: (_, row) => extensionVersion(row)
      },
      {
        title: t('auto.system_requirements'),
        key: 'host_version_requirement',
        width: 160,
        render: (_, row) => extensionHostRequirement(row) ?? '—'
      },
      {
        title: t('auto.installation'),
        key: 'installation_status',
        width: 120,
        render: (_, row) => extensionInstallationStatus(row)
      },
      {
        title: t('auto.source'),
        key: 'source',
        width: 160,
        render: (_, row) => extensionSource(row)
      },
      {
        title: t('auto.trust'),
        dataIndex: 'trust',
        key: 'trust',
        width: 120
      },
      {
        title: t('auto.operation'),
        key: 'actions',
        width: 150,
        minWidth: 150,
        align: 'center',
        render: (_, row) => {
          const key = extensionKey(row);
          const updateState = updateStates[key];
          const action = isInstalledRow(row) ? (
            <Space size={4}>
              <span data-update-state={updateState ?? 'unknown_error'}>
                <Tooltip
                  title={
                    updateState === 'update_available'
                      ? t('auto.update_available')
                      : updateState === 'current'
                        ? t('auto.currently_latest_version')
                        : t('auto.update_check_failed')
                  }
                >
                  <Badge
                    dot
                    color={
                      updateState === 'update_available'
                        ? '#ffba00'
                        : updateState === 'current'
                          ? 'transparent'
                          : '#fb565b'
                    }
                  >
                    <Button
                      type="link"
                      loading={resolvingUpdateKey === key}
                      onClick={() => void resolveInstalledUpdate(row)}
                    >
                      {t('auto.update')}
                    </Button>
                  </Badge>
                </Tooltip>
              </span>
              {row.category === 'mcp' ? (
                <Button
                  type="link"
                  onClick={() => void previewInstalledMcpExtension(row.id)}
                >
                  {t('auto.apply_to_workspace')}
                </Button>
              ) : null}
            </Space>
          ) : (
            <span data-update-state={updateState ?? 'not_installed'}>
              <Badge
                dot
                color={
                  row.installation_status === 'not_installed'
                    ? 'transparent'
                    : updateState === 'update_available'
                      ? '#ffba00'
                      : updateState === 'current'
                        ? 'transparent'
                        : '#fb565b'
                }
              >
                <Button
                  type="link"
                  loading={operationMutation.isPending}
                  onClick={() =>
                    submitOperation({
                      kind: 'catalog',
                      entry: row,
                      update: row.installation_status !== 'not_installed'
                    })
                  }
                >
                  {row.installation_status === 'not_installed'
                    ? t('auto.install')
                    : t('auto.update')}
                </Button>
              </Badge>
            </span>
          );
          return (
            <Space size={4}>
              {action}
              <Button type="link" onClick={() => setSelected(row)}>
                {t('auto.view')}
              </Button>
            </Space>
          );
        }
      }
    ],
    [
      operationMutation.isPending,
      previewInstalledMcpExtension,
      resolveInstalledUpdate,
      resolvingUpdateKey,
      submitOperation,
      t,
      updateStates
    ]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.extension_center'
  });

  const nextCursor =
    activeTab === 'installed'
      ? installedQuery.data?.next_cursor
      : catalogQuery.data?.next_cursor;
  const totalEntries =
    activeTab === 'installed'
      ? (installedQuery.data?.total_entries ?? 0)
      : (catalogQuery.data?.total_entries ?? 0);
  const tableLoading =
    activeTab === 'installed'
      ? installedQuery.isLoading || installedQuery.isFetching
      : catalogQuery.isLoading || catalogQuery.isFetching;

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey={activeTab}
          onChange={(key) => {
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category: key },
              search: { cursor: undefined }
            });
          }}
          items={[
            { key: 'installed', label: t('auto.installed_extensions') },
            ...CATEGORIES.map((category) => ({
              key: category,
              label: category
            }))
          ]}
        />
        <DataTable<ExtensionRow>
          rowKey={(row) => extensionKey(row)}
          columns={columns}
          configuration={tableConfiguration}
          dataSource={rows}
          emptyText={<Empty description={t('auto.no_extensions')} />}
          loading={tableLoading}
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
          cursorPagination={{
            currentPage: cursor ? 2 : 1,
            hasPreviousPage: Boolean(cursor),
            hasNextPage: Boolean(nextCursor),
            previousLabel: t('auto.previous_page'),
            nextLabel: t('auto.next_page'),
            total: totalEntries,
            onPreviousPage: () => {
              void navigate({
                to: '/settings/extension-center/$category',
                params: { category: activeTab },
                search: { cursor: undefined }
              });
            },
            onNextPage: () => {
              if (!nextCursor) return;
              void navigate({
                to: '/settings/extension-center/$category',
                params: { category: activeTab },
                search: { cursor: nextCursor }
              });
            }
          }}
        />
      </Flex>

      <Drawer
        open={Boolean(selected)}
        title={selected ? extensionName(selected) : undefined}
        width={560}
        onClose={() => setSelected(null)}
      >
        {selected ? (
          <Flex vertical gap={16}>
            <Descriptions column={1} bordered size="small">
              <Descriptions.Item label={t('auto.kind')}>
                {selected.category}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.description')}>
                {extensionDescription(selected) ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.current_version')}>
                {extensionVersion(selected)}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.system_requirements')}>
                {extensionHostRequirement(selected) ?? '—'}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.source')}>
                {extensionSource(selected)}
              </Descriptions.Item>
              <Descriptions.Item label={t('auto.trust')}>
                {selected.trust}
              </Descriptions.Item>
            </Descriptions>
            {isInstalledRow(selected) ? (
              <List
                bordered
                header={
                  <Typography.Text strong>
                    {t('auto.installed_versions')}
                  </Typography.Text>
                }
                dataSource={selected.installed_versions}
                renderItem={(installedVersion) => (
                  <List.Item>
                    <Descriptions column={1} size="small">
                      <Descriptions.Item label={t('auto.current_version')}>
                        {installedVersion.version}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.source')}>
                        {installedVersion.source}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.trust')}>
                        {installedVersion.trust}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.signature_status')}>
                        {installedVersion.signature_status}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.checksum')}>
                        <Typography.Text copyable ellipsis>
                          {installedVersion.checksum}
                        </Typography.Text>
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.local_path')}>
                        <Typography.Text copyable ellipsis>
                          {installedVersion.local_path}
                        </Typography.Text>
                      </Descriptions.Item>
                    </Descriptions>
                  </List.Item>
                )}
              />
            ) : null}
          </Flex>
        ) : null}
      </Drawer>
    </SettingsSectionSurface>
  );
}
