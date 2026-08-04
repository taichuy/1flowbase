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
  Input,
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
import { McpTemplateLibrary } from '../../components/mcp-management/bundle/McpTemplateLibrary';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  checkSettingsExtensionUpdates,
  deleteSettingsInstalledExtension,
  fetchSettingsExtensionCatalog,
  fetchSettingsExtensionCatalogEntry,
  fetchSettingsInstalledExtensions,
  getSettingsExtensionRiskChallenge,
  installSettingsExtension,
  settingsExtensionCatalogQueryKey,
  settingsInstalledExtensionsQueryKey,
  type SettingsExtensionCatalogEntry,
  type SettingsExtensionCategory,
  type SettingsExtensionCenterCategory,
  type SettingsInstalledExtension
} from '../../api/extensions';
import { settingsI18nCatalogQueryKey } from '../../api/i18n-catalog';
import { settingsMcpCatalogQueryKey } from '../../api/mcp-management';
import {
  ExtensionApplicationFlow,
  type ExtensionApplicationTarget
} from '../../components/extension-center/ExtensionApplicationFlow';
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
  return isInstalledRow(row) ? row.source_kind : row.catalog_source;
}

function extensionDescription(row: ExtensionRow) {
  return isInstalledRow(row) ? null : row.description;
}

function extensionInstallationStatus(row: ExtensionRow) {
  return isInstalledRow(row) ? row.status : row.installation_status;
}

function extensionApplicationStatusLabel(
  status: SettingsInstalledExtension['application_status'],
  t: (key: string) => string
) {
  switch (status) {
    case 'not_required':
      return t('auto.extension_application_not_required');
    case 'not_applied':
      return t('auto.extension_application_not_applied');
    case 'applied':
      return t('auto.extension_application_applied');
    case 'available':
      return t('auto.extension_application_available');
  }
}

function extensionKey(row: ExtensionRow) {
  return extensionCatalogId(row);
}

function McpExtensionCenterSection() {
  const { t } = useTranslation('settings');
  const navigate = useNavigate();

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey="mcp"
          tabBarExtraContent={
            <Typography.Link href="/settings/mcp-management?tab=instances">
              {t('auto.go_to_mcp_management')}
            </Typography.Link>
          }
          onChange={(key) => {
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category: key },
              search: { q: undefined, cursor: undefined }
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
        <McpTemplateLibrary variant="compact" />
      </Flex>
    </SettingsSectionSurface>
  );
}

export function SettingsExtensionCenterSection(props: {
  category: SettingsExtensionCenterCategory;
  cursor?: string;
  q?: string;
}) {
  if (props.category === 'mcp') {
    return <McpExtensionCenterSection />;
  }

  return <GenericExtensionCenterSection {...props} />;
}

function GenericExtensionCenterSection({
  category: activeTab,
  cursor,
  q
}: {
  category: SettingsExtensionCenterCategory;
  cursor?: string;
  q?: string;
}) {
  const { t } = useTranslation('settings');
  const navigate = useNavigate();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<ExtensionRow | null>(null);
  const [updateStates, setUpdateStates] = useState<Record<string, UpdateState>>(
    {}
  );
  const [activeOperationKey, setActiveOperationKey] = useState<string | null>(
    null
  );
  const [applicationTarget, setApplicationTarget] =
    useState<ExtensionApplicationTarget | null>(null);
  const [searchText, setSearchText] = useState(q ?? '');

  useEffect(() => {
    setSelected(null);
    setUpdateStates({});
    setSearchText(q ?? '');
  }, [activeTab, cursor, q]);

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
        : settingsExtensionCatalogQueryKey(activeTab, {
            q,
            slot_code: undefined,
            cursor
          }),
    queryFn: async () => {
      const category = activeTab;
      if (category === 'installed') throw new Error('catalog tab required');
      const page = await fetchSettingsExtensionCatalog(category, {
        q,
        slot_code: undefined,
        cursor
      });
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
    [
      activeTab,
      catalogQuery.data?.category,
      catalogQuery.data?.entries,
      installedQuery.data?.entries
    ]
  );

  const checkVisibleUpdates = useCallback(async () => {
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

    setUpdateStates((current) => ({
      ...current,
      ...Object.fromEntries(
        checkableRows.map((row) => [extensionKey(row), 'checking' as const])
      )
    }));
    const results = await Promise.all(
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
    );
    setUpdateStates((current) => ({
      ...current,
      ...Object.fromEntries(results.flat())
    }));
  }, [activeTab, catalogQuery.data?.catalog_page, csrfToken, rows]);

  const invalidateExtensionApplicationState = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      }),
      queryClient.invalidateQueries({ queryKey: settingsMcpCatalogQueryKey }),
      queryClient.invalidateQueries({ queryKey: settingsI18nCatalogQueryKey })
    ]);
  }, [queryClient]);
  const closeApplicationFlow = useCallback(
    () => setApplicationTarget(null),
    []
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
          onCancel: () => setActiveOperationKey(null),
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

      try {
        message.success(t('auto.extension_operation_completed'));
        await invalidateExtensionApplicationState();
        if (
          result &&
          ['import_agent_flow', 'import_mcp', 'activate_i18n'].includes(
            result.application_action
          )
        ) {
          setApplicationTarget({
            installationId: result.installation.id,
            action: result.application_action
          });
        }
      } finally {
        setActiveOperationKey(null);
      }
    },
    onError: () => {
      setActiveOperationKey(null);
      message.error(t('auto.extension_operation_failed'));
    }
  });
  const deleteVersionMutation = useMutation({
    mutationFn: async (installationId: string) => {
      if (!csrfToken) throw new Error('csrf token required');
      return deleteSettingsInstalledExtension(installationId, csrfToken);
    },
    onSuccess: async () => {
      setSelected(null);
      message.success(t('auto.extension_operation_completed'));
      await invalidateExtensionApplicationState();
    },
    onError: () => message.error(t('auto.extension_operation_failed'))
  });
  const runOperation = operationMutation.mutateAsync;

  const submitOperation = useCallback(
    (operation: ExtensionOperation) => {
      setActiveOperationKey(operation.entry.id);
      void runOperation({ operation });
    },
    [runOperation]
  );

  const resolveInstalledUpdate = useCallback(
    async (row: SettingsInstalledExtension) => {
      const key = extensionKey(row);
      setActiveOperationKey(key);
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
        setActiveOperationKey(null);
        message.error(t('auto.extension_operation_failed'));
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
        width: 190,
        render: (_, row) => (
          <Space size={4} wrap>
            <Tag>{extensionInstallationStatus(row)}</Tag>
            {isInstalledRow(row) ? (
              <Tag>
                {extensionApplicationStatusLabel(row.application_status, t)}
              </Tag>
            ) : null}
          </Space>
        )
      },
      {
        title: t('auto.source'),
        key: 'source',
        width: 160,
        render: (_, row) => extensionSource(row)
      },
      {
        title: t('auto.trust'),
        key: 'trust',
        width: 120,
        render: (_, row) => (isInstalledRow(row) ? row.trust_level : row.trust)
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
                        : updateState === 'unknown_error'
                          ? t('auto.update_check_failed')
                          : t('auto.check_updates')
                  }
                >
                  <Badge
                    dot
                    color={
                      updateState === 'update_available'
                        ? '#ffba00'
                        : updateState === 'current'
                          ? 'transparent'
                          : updateState === 'unknown_error'
                            ? '#fb565b'
                            : 'transparent'
                    }
                  >
                    <Button
                      type="link"
                      loading={activeOperationKey === key}
                      disabled={
                        updateState !== 'update_available' ||
                        (activeOperationKey !== null &&
                          activeOperationKey !== key)
                      }
                      onClick={() => void resolveInstalledUpdate(row)}
                    >
                      {t('auto.sync_latest')}
                    </Button>
                  </Badge>
                </Tooltip>
              </span>
              {row.application_action === 'configure_model_provider' ? (
                <Button
                  type="link"
                  onClick={() =>
                    window.location.assign(
                      '/settings/model-providers/providers'
                    )
                  }
                >
                  {t('auto.configure_provider')}
                </Button>
              ) : row.application_action !== 'none' ? (
                <Button
                  type="link"
                  disabled={row.application_status === 'applied'}
                  onClick={() =>
                    setApplicationTarget({
                      installationId: row.id,
                      action: row.application_action
                    })
                  }
                >
                  {row.application_status === 'applied'
                    ? t('auto.extension_application_applied')
                    : row.application_action === 'activate_i18n'
                      ? t('auto.activate')
                      : t('auto.apply_to_workspace')}
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
                  loading={activeOperationKey === key}
                  disabled={
                    activeOperationKey !== null && activeOperationKey !== key
                  }
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
      activeOperationKey,
      resolveInstalledUpdate,
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
          tabBarExtraContent={
            activeTab === 'mcp' ? (
              <Typography.Link href="/settings/mcp-management?tab=instances">
                {t('auto.go_to_mcp_management')}
              </Typography.Link>
            ) : activeTab === 'i18n' ? (
              <Typography.Link href="/settings/i18n">
                {t('auto.go_to_language_management')}
              </Typography.Link>
            ) : activeTab === 'agent-flow' ? (
              <Typography.Link href="/templates">
                {t('auto.go_to_agent_flow_templates')}
              </Typography.Link>
            ) : null
          }
          onChange={(key) => {
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category: key },
              search: { q: undefined, cursor: undefined }
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
              {activeTab !== 'installed' ? (
                <Input.Search
                  allowClear
                  aria-label={t('auto.drop_down_search_installable_vendors')}
                  placeholder={t('auto.drop_down_search_installable_vendors')}
                  style={{ width: 240 }}
                  value={searchText}
                  onChange={(event) => setSearchText(event.target.value)}
                  onClear={() => {
                    void navigate({
                      to: '/settings/extension-center/$category',
                      params: { category: activeTab },
                      search: { q: undefined, cursor: undefined }
                    });
                  }}
                  onSearch={(value) => {
                    const normalizedQuery = value.trim();
                    void navigate({
                      to: '/settings/extension-center/$category',
                      params: { category: activeTab },
                      search: {
                        q: normalizedQuery || undefined,
                        cursor: undefined
                      }
                    });
                  }}
                />
              ) : null}
              <Button
                disabled={rows.length === 0}
                loading={Object.values(updateStates).some(
                  (state) => state === 'checking'
                )}
                onClick={() => void checkVisibleUpdates()}
              >
                {t('auto.check_updates')}
              </Button>
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
                search: { q, cursor: undefined }
              });
            },
            onNextPage: () => {
              if (!nextCursor) return;
              void navigate({
                to: '/settings/extension-center/$category',
                params: { category: activeTab },
                search: { q, cursor: nextCursor }
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
                {isInstalledRow(selected)
                  ? selected.trust_level
                  : selected.trust}
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
                  <List.Item
                    actions={[
                      <Tooltip
                        key="delete"
                        title={
                          installedVersion.deletable
                            ? undefined
                            : installedVersion.delete_reasons.join(', ')
                        }
                      >
                        <Button
                          type="link"
                          danger
                          disabled={!installedVersion.deletable}
                          loading={
                            deleteVersionMutation.isPending &&
                            deleteVersionMutation.variables ===
                              installedVersion.id
                          }
                          onClick={() =>
                            Modal.confirm({
                              title: t('auto.confirm_delete'),
                              content: installedVersion.version,
                              okText: t('auto.delete'),
                              cancelText: t('auto.cancel'),
                              okButtonProps: { danger: true },
                              onOk: () =>
                                deleteVersionMutation.mutateAsync(
                                  installedVersion.id
                                )
                            })
                          }
                        >
                          {t('auto.delete')}
                        </Button>
                      </Tooltip>
                    ]}
                  >
                    <Descriptions column={1} size="small">
                      <Descriptions.Item label={t('auto.current_version')}>
                        {installedVersion.version}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.source')}>
                        {installedVersion.source_kind}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.trust')}>
                        {installedVersion.trust_level}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.signature_status')}>
                        {installedVersion.signature_status}
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.checksum')}>
                        <Typography.Text copyable ellipsis>
                          {installedVersion.local_checksum ??
                            installedVersion.expected_checksum ??
                            '—'}
                        </Typography.Text>
                      </Descriptions.Item>
                      <Descriptions.Item label={t('auto.local_path')}>
                        <Typography.Text copyable ellipsis>
                          {installedVersion.local_path ?? '—'}
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
      <ExtensionApplicationFlow
        target={applicationTarget}
        csrfToken={csrfToken ?? ''}
        onClose={closeApplicationFlow}
        onApplied={invalidateExtensionApplicationState}
      />
    </SettingsSectionSurface>
  );
}
