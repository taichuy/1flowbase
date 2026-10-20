import { extensionOperationErrorText } from '../../lib/extension-operation-error';
import { extensionRiskWarningText } from '../../lib/extension-risk-warning';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Badge, Button, Modal, Space, Switch, Tag, Tooltip, App } from 'antd';
import { useTranslation } from 'react-i18next';
import { settingsConsoleNavigationQueryKey } from '../../api/console-navigation';
import { useAuthStore } from '../../../../state/auth-store';

import { type DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import {
  checkSettingsExtensionUpdates,
  deleteSettingsInstalledExtension,
  disableSettingsInstalledExtension,
  enableSettingsInstalledExtension,
  fetchSettingsExtensionCatalog,
  fetchSettingsExtensionCatalogEntry,
  fetchSettingsInstalledExtensions,
  getSettingsExtensionRiskChallenge,
  installSettingsExtension,
  settingsExtensionCatalogQueryKey,
  settingsInstalledExtensionsQueryKey,
  type SettingsExtensionCategory,
  type SettingsExtensionCenterCategory,
  type SettingsInstalledExtension
} from '../../api/extensions/extensions';
import {
  activateSettingsInstalledI18nCatalog,
  previewSettingsInstalledI18nCatalog,
  settingsI18nCatalogQueryKey
} from '../../api/i18n-catalog';
import {
  settingsMcpCatalogQueryKey,
  settingsMcpInterfaceCapabilitiesQueryKey
} from '../../api/mcp-management';
import {
  pluginSettingsPagesQueryKey,
  settingsUiTemplatesQueryKey
} from '../../api/ui-management';
import type { ExtensionApplicationTarget } from '../../components/extension-center/ExtensionApplicationFlow';

export function useExtensionCenterController({
  category: activeTab,
  cursor,
  q
}: {
  category: Exclude<
    SettingsExtensionCenterCategory,
    'model-pricing' | 'ui-components' | 'application-templates'
  >;
  cursor?: string;
  q?: string;
}) {
  const { message } = App.useApp();
  const { t } = useTranslation('settings');
  const navigate = useNavigate();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<ExtensionRow | null>(null);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [updateStates, setUpdateStates] = useState<Record<string, UpdateState>>(
    {}
  );
  const [activeOperationKey, setActiveOperationKey] = useState<string | null>(
    null
  );
  const [applicationTarget, setApplicationTarget] =
    useState<ExtensionApplicationTarget | null>(null);
  const [pendingI18nUpdate, setPendingI18nUpdate] =
    useState<ExtensionOperation | null>(null);
  const [searchText, setSearchText] = useState(q ?? '');
  const updateCheckRequestRef = useRef(0);

  useEffect(() => {
    updateCheckRequestRef.current += 1;
    setSelected(null);
    setUpdateStates({});
    setSearchText(q ?? '');
  }, [activeTab, cursor, q]);

  const installedQuery = useQuery({
    queryKey: settingsInstalledExtensionsQueryKey(
      activeTab === 'installed' ? cursor : undefined,
      activeTab === 'installed' ? undefined : activeTab
    ),
    queryFn: () =>
      fetchSettingsInstalledExtensions(
        activeTab === 'installed' ? cursor : undefined,
        activeTab === 'installed' ? undefined : activeTab
      ),
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
    enabled: activeTab !== 'installed' && installedQuery.isSuccess,
    retry: false
  });

  const rows: ExtensionRow[] = useMemo(
    () =>
      activeTab === 'installed'
        ? (installedQuery.data?.entries ?? [])
        : catalogQuery.data?.category === activeTab
          ? catalogQuery.data.entries
          : (installedQuery.data?.entries ?? []),
    [
      activeTab,
      catalogQuery.data?.category,
      catalogQuery.data?.entries,
      installedQuery.data?.entries
    ]
  );
  const updateRows =
    activeTab === 'installed' ? rows : (installedQuery.data?.entries ?? []);

  const checkVisibleUpdates = useCallback(
    async (candidateRows: ExtensionRow[]) => {
      if (!csrfToken || candidateRows.length === 0) return;

      const checkableRows = candidateRows.filter(
        (row) =>
          (isInstalledRow(row)
            ? row.status !== 'uninstalled'
            : row.installation_status === 'installed' &&
              row.current_version !== null) &&
          (isInstalledRow(row) || row.catalog_source !== 'builtin')
      );
      const groups = new Map<SettingsExtensionCategory, ExtensionRow[]>();
      for (const row of checkableRows) {
        const group = groups.get(row.category) ?? [];
        group.push(row);
        groups.set(row.category, group);
      }
      if (groups.size === 0) return;

      const requestId = ++updateCheckRequestRef.current;
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
      if (updateCheckRequestRef.current !== requestId) return;
      setUpdateStates((current) => ({
        ...current,
        ...Object.fromEntries(results.flat())
      }));
    },
    [csrfToken]
  );

  useEffect(() => {
    if (activeTab === 'installed' || !installedQuery.isSuccess) return;
    void checkVisibleUpdates(installedQuery.data.entries);
  }, [
    activeTab,
    checkVisibleUpdates,
    installedQuery.data,
    installedQuery.isSuccess
  ]);

  const invalidateExtensionApplicationState = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: settingsConsoleNavigationQueryKey
      }),
      queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      }),
      queryClient.invalidateQueries({ queryKey: settingsMcpCatalogQueryKey }),
      queryClient.invalidateQueries({
        queryKey: settingsMcpInterfaceCapabilitiesQueryKey
      }),
      queryClient.invalidateQueries({ queryKey: settingsUiTemplatesQueryKey }),
      queryClient.invalidateQueries({ queryKey: pluginSettingsPagesQueryKey }),
      queryClient.invalidateQueries({
        queryKey: ['settings', 'console-policy-catalog']
      }),
      queryClient.invalidateQueries({ queryKey: ['settings', 'docs'] }),
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
        if (
          operation.activateI18nAfterInstall &&
          result.application_action === 'activate_i18n'
        ) {
          try {
            const preview = await previewSettingsInstalledI18nCatalog(
              result.installation.id
            );
            await activateSettingsInstalledI18nCatalog(
              result.installation.id,
              {
                expected_revision: preview.revision,
                ...(preview.required_integrity_override
                  ? {
                      integrity_override: {
                        reason: 'user_confirmed',
                        acknowledged_warnings:
                          preview.required_integrity_override.warnings.map(
                            (warning) => warning.code
                          )
                      }
                    }
                  : {})
              },
              csrfToken
            );
            return {
              challenge: null,
              operation,
              result,
              i18nActivation: 'activated' as const
            };
          } catch {
            return {
              challenge: null,
              operation,
              result,
              i18nActivation: 'failed' as const
            };
          }
        }
        return {
          challenge: null,
          operation,
          result,
          i18nActivation: 'not_requested' as const
        };
      } catch (error) {
        const challenge = getSettingsExtensionRiskChallenge(error);
        if (!challenge) throw error;
        return {
          challenge,
          operation,
          result: null,
          i18nActivation: 'not_requested' as const
        };
      }
    },
    onSuccess: async ({ challenge, operation, result, i18nActivation }) => {
      if (challenge) {
        const acknowledgedWarnings = challenge.warnings
          .filter((warning) => warning.overridable)
          .map((warning) => warning.code);
        Modal.confirm({
          title: t('auto.risk_warnings'),
          content: (
            <ul className="structured-list__items structured-list--small">
              {challenge.warnings.map((warning, index) => (
                <li
                  className="structured-list__item"
                  key={`${warning.code}-${index}`}
                >
                  {extensionRiskWarningText(warning.code, warning.message, t)}
                </li>
              ))}
            </ul>
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
        if (i18nActivation === 'failed') {
          message.error(
            t('auto.translation_catalog_activation_failed_after_install')
          );
        } else if (i18nActivation === 'activated') {
          message.success(t('auto.translation_catalog_activated'));
        } else {
          message.success(
            operation.entry.category === 'i18n' &&
              operation.activateI18nAfterInstall === false
              ? t('auto.translation_catalog_installed_not_activated')
              : t('auto.extension_operation_completed')
          );
        }
        await invalidateExtensionApplicationState();
        if (
          result &&
          ['import_agent_flow', 'import_mcp', 'activate_i18n'].includes(
            result.application_action
          ) &&
          !(
            result.application_action === 'activate_i18n' &&
            operation.activateI18nAfterInstall !== undefined
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
    onError: (error) => {
      setActiveOperationKey(null);
      message.error(extensionOperationErrorText(error, t));
    }
  });
  const deleteInstalledExtensionMutation = useMutation({
    mutationFn: async (installationId: string) => {
      if (!csrfToken) throw new Error('csrf token required');
      return deleteSettingsInstalledExtension(installationId, csrfToken);
    },
    onSuccess: async () => {
      setSelected(null);
      message.success(t('auto.extension_operation_completed'));
      await invalidateExtensionApplicationState();
    },
    onError: (error) => message.error(extensionOperationErrorText(error, t))
  });
  const activationMutation = useMutation({
    mutationFn: async ({
      installationId,
      enabled
    }: {
      installationId: string;
      enabled: boolean;
    }) => {
      if (!csrfToken) throw new Error('csrf token required');
      return enabled
        ? enableSettingsInstalledExtension(installationId, csrfToken)
        : disableSettingsInstalledExtension(installationId, csrfToken);
    },
    onSuccess: async () => {
      message.success(t('auto.extension_operation_completed'));
      await invalidateExtensionApplicationState();
    },
    onError: (error) => message.error(extensionOperationErrorText(error, t))
  });
  const deleteInstalledExtension = deleteInstalledExtensionMutation.mutate;
  const deleteInstalledExtensionAsync =
    deleteInstalledExtensionMutation.mutateAsync;
  const deletingInstalledExtensionId =
    deleteInstalledExtensionMutation.isPending
      ? deleteInstalledExtensionMutation.variables
      : null;
  const toggleInstalledExtensionActivation = activationMutation.mutate;
  const activatingInstallationId = activationMutation.isPending
    ? activationMutation.variables?.installationId
    : null;
  const runOperation = operationMutation.mutate;

  const submitOperation = useCallback(
    (operation: ExtensionOperation) => {
      setActiveOperationKey(operation.entry.id);
      runOperation({ operation });
    },
    [runOperation]
  );

  const requestOperation = useCallback(
    (operation: ExtensionOperation) => {
      if (operation.update && operation.entry.category === 'i18n') {
        setPendingI18nUpdate(operation);
        setActiveOperationKey(null);
        return;
      }
      submitOperation(operation);
    },
    [submitOperation]
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
        requestOperation({ kind: 'catalog', entry, update: true });
      } catch {
        setUpdateStates((current) => ({
          ...current,
          [key]: 'unknown_error'
        }));
        setActiveOperationKey(null);
        message.error(t('auto.extension_operation_failed'));
      }
    },
    [message, requestOperation, t]
  );
  const resolveInstalledReinstall = useCallback(
    async (row: SettingsInstalledExtension) => {
      const key = extensionKey(row);
      setActiveOperationKey(key);
      try {
        const entry = await fetchSettingsExtensionCatalogEntry(
          row.category,
          row.catalog_id
        );
        requestOperation({ kind: 'catalog', entry, update: false });
      } catch {
        setActiveOperationKey(null);
        message.error(t('auto.extension_operation_failed'));
      }
    },
    [message, requestOperation, t]
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
        title:
          activeTab === 'installed'
            ? t('auto.current_version')
            : t('auto.latest_version'),
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
            <Tag>{extensionInstallationStatusLabel(row, t)}</Tag>
            {isInstalledRow(row) ? (
              <>
                <Tag>{row.desired_state ?? '—'}</Tag>
                <Tag
                  data-testid="plugin-runtime-status"
                  data-runtime-status={row.runtime_status ?? undefined}
                >
                  {row.runtime_status === 'active'
                    ? t('auto.running')
                    : row.runtime_status === 'inactive'
                      ? t('auto.plugin_runtime_inactive')
                      : row.runtime_status === 'load_failed'
                        ? t('auto.plugin_runtime_load_failed')
                        : (row.runtime_status ?? '—')}
                </Tag>
                <Tag
                  data-testid="plugin-availability-status"
                  data-availability-status={
                    row.availability_status ?? undefined
                  }
                >
                  {row.availability_status === 'pending_restart'
                    ? t('auto.plugin_pending_restart')
                    : (row.availability_status ?? '—')}
                </Tag>
                <Tag>
                  {extensionApplicationStatusLabel(row.application_status, t)}
                </Tag>
              </>
            ) : null}
          </Space>
        )
      },
      ...(activeTab === 'installed'
        ? [
            {
              title: t('auto.enabled'),
              key: 'enabled',
              width: 100,
              align: 'center' as const,
              render: (_: unknown, row: ExtensionRow) => {
                if (
                  !isInstalledRow(row) ||
                  ![
                    'runtime-extensions',
                    'capability-plugins',
                    'host-extensions'
                  ].includes(row.category)
                ) {
                  return '—';
                }
                const control = (
                  <Switch
                    aria-label={`${extensionName(row)} ${t('auto.enabled')}`}
                    checked={
                      row.status !== 'uninstalled' &&
                      row.desired_state !== 'disabled'
                    }
                    disabled={row.status === 'uninstalled'}
                    loading={activatingInstallationId === row.id}
                    onChange={(enabled) =>
                      toggleInstalledExtensionActivation({
                        installationId: row.id,
                        enabled
                      })
                    }
                  />
                );
                return row.category === 'host-extensions' ? (
                  <Tooltip
                    title={t('auto.extension_activation_requires_restart')}
                  >
                    {control}
                  </Tooltip>
                ) : (
                  control
                );
              }
            }
          ]
        : []),
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
          const updateState = updateStates[key] ?? 'unchecked';
          const action = isInstalledRow(row) ? (
            <Space size={4}>
              {row.status !== 'uninstalled' ? (
                <span data-update-state={updateState}>
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
              ) : null}
              {isFamilyUninstallRow(row) && row.status === 'uninstalled' ? (
                <Button
                  type="link"
                  loading={activeOperationKey === key}
                  onClick={() => void resolveInstalledReinstall(row)}
                >
                  {t('auto.reinstall')}
                </Button>
              ) : isFamilyUninstallRow(row) ? (
                <Button
                  danger
                  type="link"
                  loading={deletingInstalledExtensionId === row.id}
                  disabled={
                    deletingInstalledExtensionId !== null &&
                    deletingInstalledExtensionId !== row.id
                  }
                  onClick={() => deleteInstalledExtension(row.id)}
                >
                  {t('auto.uninstall_plugin')}
                </Button>
              ) : null}
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
          ) : row.catalog_source === 'builtin' ? null : (
            <span
              data-update-state={
                row.installation_status !== 'installed'
                  ? row.installation_status
                  : updateState
              }
            >
              <Badge
                dot
                color={
                  row.installation_status !== 'installed'
                    ? 'transparent'
                    : updateState === 'update_available'
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
                    activeOperationKey !== null && activeOperationKey !== key
                  }
                  onClick={() =>
                    requestOperation({
                      kind: 'catalog',
                      entry: row,
                      update: row.installation_status === 'installed'
                    })
                  }
                >
                  {row.installation_status === 'installed'
                    ? t('auto.update')
                    : row.installation_status === 'uninstalled'
                      ? t('auto.reinstall')
                      : t('auto.install')}
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
      activeTab,
      activatingInstallationId,
      deleteInstalledExtension,
      deletingInstalledExtensionId,
      resolveInstalledReinstall,
      resolveInstalledUpdate,
      requestOperation,
      t,
      toggleInstalledExtensionActivation,
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
      : (catalogQuery.data?.total_entries ??
        installedQuery.data?.total_entries ??
        0);
  const tableLoading =
    activeTab === 'installed'
      ? installedQuery.isLoading || installedQuery.isFetching
      : installedQuery.isLoading ||
        (catalogQuery.isLoading && rows.length === 0);
  return {
    t,
    navigate,
    csrfToken,
    selected,
    setSelected,
    uploadOpen,
    setUploadOpen,
    updateStates,
    applicationTarget,
    setApplicationTarget,
    pendingI18nUpdate,
    setPendingI18nUpdate,
    searchText,
    setSearchText,
    catalogQuery,
    rows,
    updateRows,
    checkVisibleUpdates,
    invalidateExtensionApplicationState,
    closeApplicationFlow,
    deleteInstalledExtensionAsync,
    deletingInstalledExtensionId,
    toggleInstalledExtensionActivation,
    activatingInstallationId,
    submitOperation,
    columns,
    tableConfiguration,
    nextCursor,
    totalEntries,
    tableLoading
  };
}

import {
  type ExtensionRow,
  type UpdateState,
  type ExtensionOperation,
  type ExtensionOverrides,
  isInstalledRow,
  isFamilyUninstallRow,
  extensionCatalogId,
  extensionName,
  extensionVersion,
  extensionHostRequirement,
  extensionSource,
  extensionDescription,
  extensionInstallationStatusLabel,
  extensionApplicationStatusLabel,
  extensionKey
} from './extension-center-presentation';
