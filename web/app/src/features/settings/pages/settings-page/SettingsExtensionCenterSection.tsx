import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { ApiClientError } from '@1flowbase/api-client';
import {
  Alert,
  Badge,
  Button,
  Descriptions,
  Drawer,
  Empty,
  Flex,
  Input,
  Modal,
  Space,
  Switch,
  Tabs,
  Tag,
  Tooltip,
  Typography,
  App
} from 'antd';
import { useTranslation } from 'react-i18next';
import '../../../../shared/ui/structured-list/structured-list.css';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
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
  type SettingsExtensionCatalogEntry,
  type SettingsExtensionCategory,
  type SettingsExtensionCenterCategory,
  type SettingsInstalledExtension
} from '../../api/extensions';
import {
  activateSettingsInstalledI18nCatalog,
  previewSettingsInstalledI18nCatalog,
  settingsI18nCatalogQueryKey
} from '../../api/i18n-catalog';
import { settingsMcpCatalogQueryKey } from '../../api/mcp-management';
import type { ExtensionApplicationTarget } from '../../components/extension-center/ExtensionApplicationFlow';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

const PricingCatalogPanel = lazy(() =>
  import('../../components/billing/PricingCatalogPanel').then((module) => ({
    default: module.PricingCatalogPanel
  }))
);
const UiComponentCatalogPanel = lazy(() =>
  import('../../components/extension-center/UiComponentCatalogPanel').then(
    (module) => ({ default: module.UiComponentCatalogPanel })
  )
);
const ExtensionApplicationFlow = lazy(() =>
  import('../../components/extension-center/ExtensionApplicationFlow').then(
    (module) => ({ default: module.ExtensionApplicationFlow })
  )
);

type ExtensionRow = SettingsInstalledExtension | SettingsExtensionCatalogEntry;
type UpdateState =
  | 'unchecked'
  | 'checking'
  | 'current'
  | 'update_available'
  | 'unknown_error';
type ExtensionOperation = {
  kind: 'catalog';
  entry: SettingsExtensionCatalogEntry;
  update: boolean;
  activateI18nAfterInstall?: boolean;
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

function supportsFamilyUninstall(category: SettingsExtensionCategory) {
  return category === 'runtime-extensions' || category === 'capability-plugins';
}

function isFamilyUninstallRow(
  row: ExtensionRow
): row is SettingsInstalledExtension {
  return isInstalledRow(row) && supportsFamilyUninstall(row.category);
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

function extensionInstallationStatusLabel(
  row: ExtensionRow,
  t: (key: string) => string
) {
  return extensionInstallationStatus(row) === 'uninstalled'
    ? t('auto.uninstalled')
    : extensionInstallationStatus(row);
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

function mcpTemplateWorkspaceStatusLabel(
  status: SettingsExtensionCatalogEntry['mcp_instances'][number]['workspace_status'],
  t: (key: string) => string
) {
  switch (status) {
    case 'applied':
      return t('auto.extension_application_applied');
    case 'missing':
      return t('auto.revoked');
    case 'modified':
      return t('auto.mcp_template_instance_status_modified');
  }
}

function extensionKey(row: ExtensionRow) {
  return extensionCatalogId(row);
}

function extensionOperationErrorKey(error: unknown) {
  if (!(error instanceof ApiClientError))
    return 'auto.extension_operation_failed';
  switch (error.code) {
    case 'extension_artifact_not_published':
      return 'auto.extension_artifact_not_published';
    case 'extension_artifact_network_unavailable':
    case 'extension_artifact_upstream_rejected':
    case 'extension_artifact_download_unavailable':
      return 'auto.extension_artifact_download_failed';
    case 'extension_artifact_checksum_mismatch':
      return 'auto.extension_artifact_checksum_failed';
    case 'extension_artifact_signature_invalid':
      return 'auto.extension_artifact_signature_failed';
    default:
      return 'auto.extension_operation_failed';
  }
}

function extensionRiskWarningKey(code: string) {
  switch (code) {
    case 'checksum_missing':
      return 'auto.extension_warning_checksum_missing';
    case 'signature_missing':
      return 'auto.extension_warning_signature_missing';
    case 'signing_key_unknown':
      return 'auto.extension_warning_signing_key_unknown';
    default:
      return null;
  }
}

function extensionRiskWarningText(
  code: string,
  message: string,
  t: (key: string) => string
) {
  const key = extensionRiskWarningKey(code);
  return key ? t(key) : message;
}

export function SettingsExtensionCenterSection(props: {
  category: SettingsExtensionCenterCategory;
  cursor?: string;
  q?: string;
  canManageUiComponents?: boolean;
}) {
  if (props.category === 'model-pricing') {
    return (
      <Suspense fallback={null}>
        <PricingCatalogPanel />
      </Suspense>
    );
  }
  if (props.category === 'ui-components') {
    return (
      <Suspense fallback={null}>
        <UiComponentCatalogPanel
          canManage={props.canManageUiComponents ?? false}
        />
      </Suspense>
    );
  }
  return (
    <GenericExtensionCenterSection
      category={props.category}
      cursor={props.cursor}
      q={props.q}
    />
  );
}

function GenericExtensionCenterSection({
  category: activeTab,
  cursor,
  q
}: {
  category: Exclude<
    SettingsExtensionCenterCategory,
    'model-pricing' | 'ui-components'
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
      message.error(t(extensionOperationErrorKey(error)));
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
    onError: () => message.error(t('auto.extension_operation_failed'))
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
    onError: () => message.error(t('auto.extension_operation_failed'))
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
                <Tag>{row.availability_status ?? '—'}</Tag>
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
            })),
            {
              key: 'ui-components',
              label: t('auto.ui_components')
            },
            {
              key: 'model-pricing',
              label: t('auto.billing_vendor_model_pricing')
            }
          ]}
        />
        {activeTab !== 'installed' && catalogQuery.isError ? (
          <Alert
            type="error"
            showIcon
            title={t('auto.extension_catalog_load_failed')}
            description={t('auto.extension_catalog_load_failed_description')}
            action={
              <Button onClick={() => void catalogQuery.refetch()}>
                {t('auto.extension_catalog_retry')}
              </Button>
            }
          />
        ) : null}
        {activeTab !== 'installed' &&
        catalogQuery.data?.freshness === 'stale' ? (
          <Alert
            type="warning"
            showIcon
            title={t('auto.extension_catalog_stale')}
          />
        ) : null}
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
                disabled={updateRows.length === 0}
                loading={Object.values(updateStates).some(
                  (state) => state === 'checking'
                )}
                onClick={() => void checkVisibleUpdates(updateRows)}
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
        size={560}
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
            {!isInstalledRow(selected) &&
            selected.category === 'mcp' &&
            selected.mcp_instances.length > 0 ? (
              <div className="structured-list structured-list--bordered">
                <div className="structured-list__header">
                  <Typography.Text strong>
                    {t('auto.mcp_instances')}
                  </Typography.Text>
                </div>
                <ul className="structured-list__items">
                  {selected.mcp_instances.map((instance) => (
                    <li
                      className="structured-list__item"
                      key={instance.instance_id}
                    >
                      <div className="structured-list__content">
                        <Flex vertical gap={4}>
                          <Space size={8} wrap>
                            <Typography.Text strong>
                              {instance.name}
                            </Typography.Text>
                            <Tag>
                              {mcpTemplateWorkspaceStatusLabel(
                                instance.workspace_status,
                                t
                              )}
                            </Tag>
                          </Space>
                          <Typography.Text type="secondary">
                            {instance.description_short ?? instance.instance_id}
                          </Typography.Text>
                        </Flex>
                      </div>
                      <div className="structured-list__actions">
                        <Button
                          type="link"
                          disabled={!selected.builtin_template_id}
                          onClick={() => {
                            if (!selected.builtin_template_id) return;
                            setApplicationTarget({
                              builtinTemplateId: selected.builtin_template_id,
                              instanceId: instance.instance_id,
                              action: 'import_mcp'
                            });
                          }}
                        >
                          {i18nText(
                            'settingsMcpManagement',
                            'auto.restore_instance_default'
                          )}
                        </Button>
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            {isInstalledRow(selected) ? (
              <div className="structured-list structured-list--bordered">
                <div className="structured-list__header">
                  <Typography.Text strong>
                    {t('auto.installed_versions')}
                  </Typography.Text>
                </div>
                <ul className="structured-list__items">
                  {selected.installed_versions.map((installedVersion) => (
                    <li
                      className="structured-list__item"
                      key={installedVersion.id}
                    >
                      <div className="structured-list__content">
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
                      </div>
                      {!supportsFamilyUninstall(selected.category) ? (
                        <div className="structured-list__actions">
                          <Tooltip
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
                                deletingInstalledExtensionId ===
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
                                    deleteInstalledExtensionAsync(
                                      installedVersion.id
                                    )
                                })
                              }
                            >
                              {t('auto.delete')}
                            </Button>
                          </Tooltip>
                        </div>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </Flex>
        ) : null}
      </Drawer>
      <Modal
        open={Boolean(pendingI18nUpdate)}
        title={t('auto.update_translation_catalog')}
        onCancel={() => setPendingI18nUpdate(null)}
        footer={
          <Space>
            <Button onClick={() => setPendingI18nUpdate(null)}>
              {t('auto.cancel')}
            </Button>
            <Button
              onClick={() => {
                if (!pendingI18nUpdate) return;
                const operation = pendingI18nUpdate;
                setPendingI18nUpdate(null);
                submitOperation({
                  ...operation,
                  activateI18nAfterInstall: false
                });
              }}
            >
              {t('auto.install_new_version_only')}
            </Button>
            <Button
              type="primary"
              onClick={() => {
                if (!pendingI18nUpdate) return;
                const operation = pendingI18nUpdate;
                setPendingI18nUpdate(null);
                submitOperation({
                  ...operation,
                  activateI18nAfterInstall: true
                });
              }}
            >
              {t('auto.install_and_activate')}
            </Button>
          </Space>
        }
      >
        <Typography.Paragraph>
          {t('auto.translation_catalog_update_choice_description')}
        </Typography.Paragraph>
      </Modal>
      {applicationTarget ? (
        <Suspense fallback={null}>
          <ExtensionApplicationFlow
            target={applicationTarget}
            csrfToken={csrfToken ?? ''}
            onClose={closeApplicationFlow}
            onApplied={invalidateExtensionApplicationState}
          />
        </Suspense>
      ) : null}
    </SettingsSectionSurface>
  );
}
