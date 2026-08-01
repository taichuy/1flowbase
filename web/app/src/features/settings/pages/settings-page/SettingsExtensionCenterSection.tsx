import { useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
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
  Table,
  Tabs,
  Tag,
  Tooltip,
  Typography,
  Upload,
  message,
  type TableColumnsType,
  type UploadFile
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  checkSettingsExtensionUpdates,
  fetchSettingsExtensionCatalog,
  fetchSettingsExtensionCatalogEntry,
  fetchSettingsInstalledExtensions,
  getSettingsExtensionRiskChallenge,
  installSettingsExtension,
  settingsExtensionCatalogQueryKey,
  settingsInstalledExtensionsQueryKey,
  uploadSettingsExtension,
  type SettingsExtensionCatalogEntry,
  type SettingsExtensionCategory,
  type SettingsInstalledExtension
} from '../../api/extensions';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

type ExtensionRow = SettingsInstalledExtension | SettingsExtensionCatalogEntry;
type UpdateState =
  | 'checking'
  | 'current'
  | 'update_available'
  | 'unknown_error';
type ExtensionOperation =
  | {
      kind: 'catalog';
      entry: SettingsExtensionCatalogEntry;
      update: boolean;
    }
  | { kind: 'upload'; file: File };

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

function extensionKey(row: Pick<ExtensionRow, 'category' | 'artifact_id'>) {
  return `${row.category}:${row.artifact_id}`;
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
  const [resolvingUpdateKey, setResolvingUpdateKey] = useState<string | null>(
    null
  );
  const [uploadOpen, setUploadOpen] = useState(false);
  const [uploadFiles, setUploadFiles] = useState<UploadFile[]>([]);

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

  const rows: ExtensionRow[] = useMemo(
    () =>
      activeTab === 'installed'
        ? (installedQuery.data?.entries ?? [])
        : (catalogQuery.data?.entries ?? []),
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
                artifact_id: entry.artifact_id,
                current_version: isInstalledRow(entry)
                  ? entry.current_version
                  : entry.current_version!
              }))
            },
            csrfToken
          );
          return result.items.map(
            (item) => [`${category}:${item.artifact_id}`, item.status] as const
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

  const operationMutation = useMutation({
    mutationFn: async ({
      operation,
      overrides = {}
    }: {
      operation: ExtensionOperation;
      overrides?: Parameters<typeof uploadSettingsExtension>[2];
    }) => {
      if (!csrfToken) throw new Error('csrf token required');
      try {
        if (operation.kind === 'upload') {
          await uploadSettingsExtension(operation.file, csrfToken, overrides);
        } else {
          await installSettingsExtension(
            operation.entry,
            csrfToken,
            overrides,
            operation.update
          );
        }
        return { challenge: null, operation };
      } catch (error) {
        const challenge = getSettingsExtensionRiskChallenge(error);
        if (!challenge) throw error;
        return { challenge, operation };
      }
    },
    onSuccess: async ({ challenge, operation }) => {
      if (challenge) {
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
                risk_override: {
                  reason: 'user_confirmed',
                  acknowledged_warnings: challenge.warnings.map(
                    (warning) => warning.code
                  )
                },
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
      if (operation.kind === 'upload') {
        setUploadOpen(false);
        setUploadFiles([]);
      }
      await queryClient.invalidateQueries({
        queryKey: ['settings', 'extension-center']
      });
    },
    onError: () => message.error(t('auto.extension_operation_failed'))
  });

  const submitOperation = (operation: ExtensionOperation) => {
    Modal.confirm({
      title:
        operation.kind === 'upload'
          ? t('auto.upload_plugin')
          : operation.update
            ? t('auto.update_extension')
            : t('auto.install_extension'),
      content: t('auto.extension_install_confirmation'),
      okText: t('auto.confirm'),
      cancelText: t('auto.cancel'),
      onOk: () => operationMutation.mutateAsync({ operation })
    });
  };

  const resolveInstalledUpdate = async (row: SettingsInstalledExtension) => {
    const key = extensionKey(row);
    setResolvingUpdateKey(key);
    try {
      const entry = await fetchSettingsExtensionCatalogEntry(
        row.category,
        row.artifact_id
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
  };

  const columns: TableColumnsType<ExtensionRow> = [
    {
      title: t('auto.name'),
      dataIndex: 'display_name',
      key: 'display_name',
      ellipsis: true
    },
    {
      title: t('auto.kind'),
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
      render: (_, row) => row.current_version ?? '—'
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
        const key = extensionKey(row);
        const updateState = updateStates[key];
        const action = isInstalledRow(row) ? (
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
  ];

  const nextCursor =
    activeTab === 'installed'
      ? installedQuery.data?.next_cursor
      : catalogQuery.data?.next_cursor;

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey={activeTab}
          tabBarExtraContent={
            <Button onClick={() => setUploadOpen(true)}>
              {t('auto.upload_plugin')}
            </Button>
          }
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
          rowKey={(row) => extensionKey(row)}
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
            <Descriptions.Item label={t('auto.kind')}>
              {selected.category}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.description')}>
              {selected.description ?? '—'}
            </Descriptions.Item>
            <Descriptions.Item label={t('auto.current_version')}>
              {selected.current_version ?? '—'}
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

      <Modal
        open={uploadOpen}
        title={t('auto.upload_plugin')}
        okText={t('auto.upload_and_install')}
        cancelText={t('auto.cancel')}
        confirmLoading={operationMutation.isPending}
        onCancel={() => {
          setUploadOpen(false);
          setUploadFiles([]);
        }}
        onOk={() => {
          const file = uploadFiles[0]?.originFileObj;
          if (!(file instanceof File)) {
            message.warning(t('auto.select_plug_package_first'));
            return;
          }
          submitOperation({ kind: 'upload', file });
        }}
      >
        <Typography.Paragraph type="secondary">
          {t(
            'auto.supports_one_flowbasepkg_compatible_tar_gz_zip_uploading_host_backend'
          )}
        </Typography.Paragraph>
        <Upload
          maxCount={1}
          fileList={uploadFiles}
          beforeUpload={() => false}
          onChange={({ fileList }) => setUploadFiles(fileList.slice(-1))}
        >
          <Button>{t('auto.upload_plugin')}</Button>
        </Upload>
      </Modal>
    </SettingsSectionSurface>
  );
}
