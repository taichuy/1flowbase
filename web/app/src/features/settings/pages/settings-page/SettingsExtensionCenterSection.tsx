import { useEffect, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Badge,
  Button,
  Descriptions,
  Drawer,
  Empty,
  Flex,
  Modal,
  Space,
  Table,
  Tabs,
  Tag,
  Tooltip,
  message,
  type TableColumnsType
} from 'antd';
import { useTranslation } from 'react-i18next';

import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import {
  checkSettingsExtensionUpdates,
  fetchSettingsExtensionCatalog,
  fetchSettingsInstalledExtensions,
  installSettingsExtension,
  settingsExtensionCatalogQueryKey,
  settingsInstalledExtensionsQueryKey,
  type SettingsExtensionCatalogEntry,
  type SettingsExtensionCategory,
  type SettingsInstalledExtension
} from '../../api/extensions';
import { useAuthStore } from '../../../../state/auth-store';

type ExtensionRow = SettingsInstalledExtension | SettingsExtensionCatalogEntry;
type UpdateState = 'current' | 'update_available' | 'unknown_error';

const CATEGORIES: SettingsExtensionCategory[] = [
  'agent-flow',
  'capability-plugins',
  'host-extensions',
  'i18n',
  'mcp',
  'runtime-extensions'
];

function isInstalledRow(row: ExtensionRow): row is SettingsInstalledExtension {
  return 'installation' in row;
}

export function SettingsExtensionCenterSection() {
  const { t } = useTranslation('settings');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<
    'installed' | SettingsExtensionCategory
  >('installed');
  const [cursor, setCursor] = useState<string>();
  const [cursorHistory, setCursorHistory] = useState<string[]>([]);
  const [selected, setSelected] = useState<ExtensionRow | null>(null);
  const [updateStates, setUpdateStates] = useState<Record<string, UpdateState>>(
    {}
  );

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
    queryFn: () => {
      if (activeTab === 'installed') throw new Error('catalog tab required');
      return fetchSettingsExtensionCatalog(activeTab, cursor);
    },
    enabled: activeTab !== 'installed',
    retry: false
  });

  useEffect(() => {
    if (activeTab !== 'installed' || !installedQuery.data || !csrfToken) return;
    const groups = new Map<
      SettingsExtensionCategory,
      SettingsInstalledExtension[]
    >();
    for (const entry of installedQuery.data.entries) {
      const group = groups.get(entry.category) ?? [];
      group.push(entry);
      groups.set(entry.category, group);
    }
    let cancelled = false;
    void Promise.all(
      [...groups.entries()].map(async ([category, entries]) => {
        try {
          const result = await checkSettingsExtensionUpdates(
            {
              category,
              catalog_page: null,
              items: entries.map((entry) => ({
                artifact_id: entry.artifact_id,
                current_version: entry.current_version
              }))
            },
            csrfToken
          );
          return result.items.map(
            (item) => [item.artifact_id, item.status] as const
          );
        } catch {
          return entries.map(
            (entry) => [entry.artifact_id, 'unknown_error'] as const
          );
        }
      })
    ).then((groupsResult) => {
      if (!cancelled) setUpdateStates(Object.fromEntries(groupsResult.flat()));
    });
    return () => {
      cancelled = true;
    };
  }, [activeTab, csrfToken, installedQuery.data]);

  const installMutation = useMutation({
    mutationFn: ({
      entry,
      update
    }: {
      entry: SettingsExtensionCatalogEntry;
      update: boolean;
    }) => {
      if (!csrfToken) throw new Error('csrf token required');
      const warningCodes = entry.warnings
        .filter((item) => item.overridable)
        .map((item) => item.code);
      return installSettingsExtension(
        entry,
        csrfToken,
        {
          ...(warningCodes.length > 0
            ? {
                risk_override: {
                  reason: 'user_confirmed',
                  acknowledged_warnings: warningCodes
                }
              }
            : {}),
          ...(entry.warnings.some(
            (item) => item.code === 'below_minimum_host_version'
          ) &&
          entry.current_host_version &&
          entry.minimum_host_version
            ? {
                compatibility_override: {
                  reason: 'below_minimum_host_version' as const,
                  acknowledged_current_host_version: entry.current_host_version,
                  acknowledged_minimum_host_version: entry.minimum_host_version
                }
              }
            : {})
        },
        update
      );
    },
    onSuccess: async () => {
      message.success(t('auto.extension_operation_submitted'));
      await queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      });
    },
    onError: () => message.error(t('auto.extension_operation_failed'))
  });

  const rows: ExtensionRow[] =
    activeTab === 'installed'
      ? (installedQuery.data?.entries ?? [])
      : (catalogQuery.data?.entries ?? []);
  const nextCursor =
    activeTab === 'installed'
      ? installedQuery.data?.next_cursor
      : catalogQuery.data?.next_cursor;

  const confirmInstall = (
    entry: SettingsExtensionCatalogEntry,
    update: boolean
  ) => {
    const hasWarnings = entry.warnings.length > 0;
    Modal.confirm({
      title: update ? t('auto.update_extension') : t('auto.install_extension'),
      content: hasWarnings
        ? t('auto.extension_warning_confirmation')
        : t('auto.extension_install_confirmation'),
      okText: t('auto.confirm'),
      cancelText: t('auto.cancel'),
      onOk: () => installMutation.mutateAsync({ entry, update })
    });
  };

  const columns: TableColumnsType<ExtensionRow> = [
    {
      title: t('auto.name'),
      dataIndex: 'display_name',
      key: 'display_name',
      ellipsis: true
    },
    {
      title: t('auto.type'),
      dataIndex: 'category',
      key: 'category',
      render: (value: string) => <Tag>{value}</Tag>
    },
    {
      title: t('auto.description'),
      dataIndex: 'description',
      key: 'description',
      ellipsis: true
    },
    {
      title: t('auto.current_version'),
      key: 'current_version',
      render: (_, row) =>
        isInstalledRow(row)
          ? row.current_version
          : (row.current_version ?? row.latest_version)
    },
    {
      title: t('auto.system_requirements'),
      dataIndex: 'system_requirements',
      key: 'system_requirements',
      render: (value: string | null) => value ?? '—'
    },
    {
      title: t('auto.installation'),
      dataIndex: 'installation_status',
      key: 'installation_status'
    },
    { title: t('auto.source'), dataIndex: 'source', key: 'source' },
    { title: t('auto.trust'), dataIndex: 'trust', key: 'trust' },
    {
      title: t('auto.operation'),
      key: 'actions',
      fixed: 'right',
      render: (_, row) => {
        const updateState = updateStates[row.artifact_id];
        const action = isInstalledRow(row) ? (
          <Tooltip
            title={
              updateState === 'update_available'
                ? t('auto.update_available')
                : updateState === 'unknown_error'
                  ? t('auto.update_check_failed')
                  : t('auto.current_version_is_latest')
            }
          >
            <Badge
              dot
              color={
                updateState === 'update_available'
                  ? '#ffba00'
                  : updateState === 'unknown_error'
                    ? '#fb565b'
                    : 'transparent'
              }
            >
              <Button type="link" onClick={() => setActiveTab(row.category)}>
                {t('auto.update')}
              </Button>
            </Badge>
          </Tooltip>
        ) : row.artifact_kind ? (
          <Button
            type="link"
            loading={installMutation.isPending}
            onClick={() =>
              confirmInstall(row, row.installation_status !== 'not_installed')
            }
          >
            {row.installation_status === 'not_installed'
              ? t('auto.install')
              : t('auto.update')}
          </Button>
        ) : null;
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
  ];

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey={activeTab}
          onChange={(key) => {
            setActiveTab(key as typeof activeTab);
            setCursor(undefined);
            setCursorHistory([]);
          }}
          items={[
            { key: 'installed', label: t('auto.installed_extensions') },
            ...CATEGORIES.map((category) => ({
              key: category,
              label: category
            }))
          ]}
        />
        <Table<ExtensionRow>
          rowKey="artifact_id"
          columns={columns}
          dataSource={rows}
          loading={installedQuery.isLoading || catalogQuery.isLoading}
          locale={{
            emptyText: <Empty description={t('auto.no_extensions')} />
          }}
          pagination={false}
          scroll={{ x: 1280 }}
        />
        <Flex justify="end" gap={8}>
          <Button
            disabled={cursorHistory.length === 0}
            onClick={() => {
              const history = cursorHistory.slice(0, -1);
              setCursor(history.at(-1));
              setCursorHistory(history);
            }}
          >
            {t('auto.previous_page')}
          </Button>
          <Button
            disabled={!nextCursor}
            onClick={() => {
              if (!nextCursor) return;
              setCursorHistory((history) => [...history, nextCursor]);
              setCursor(nextCursor);
            }}
          >
            {t('auto.next_page')}
          </Button>
        </Flex>
      </Flex>
      <Drawer
        open={Boolean(selected)}
        title={selected?.display_name}
        width={420}
        onClose={() => setSelected(null)}
      >
        {selected ? (
          <Descriptions column={1} bordered size="small">
            <Descriptions.Item label={t('auto.type')}>
              {selected.category}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.description')}>
              {selected.description ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.current_version')}>
              {isInstalledRow(selected)
                ? selected.current_version
                : (selected.current_version ?? selected.latest_version)}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.system_requirements')}>
              {selected.system_requirements ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.source')}>
              {selected.source}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.trust')}>
              {selected.trust}
            </Descriptions.Item>
          </Descriptions>
        ) : null}
      </Drawer>
    </SettingsSectionSurface>
  );
}
