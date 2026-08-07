import {
  useEffect,
  useCallback,
  useMemo,
  useRef,
  useState,
  type Key
} from 'react';

import { EyeOutlined, FileSearchOutlined, ReloadOutlined } from '@ant-design/icons';
import {
  Alert,
  Button,
  Descriptions,
  Drawer,
  Empty,
  Input,
  Layout,
  Space,
  Table,
  Tabs,
  Tag,
  Tree,
  Typography,
  Switch,
  Tooltip
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type { DataNode } from 'antd/es/tree';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAuthStore } from '../../../../state/auth-store';
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
import './host-infrastructure-panel.css';
import {
  fetchSettingsHostInfrastructureMemoryEntries,
  fetchSettingsHostInfrastructureMemoryOverview,
  fetchSettingsHostInfrastructureMemoryStatsOverview,
  fetchSettingsHostInfrastructureMemoryTree,
  revealSettingsHostInfrastructureMemoryEntry,
  searchSettingsHostInfrastructureMemoryEntries,
  settingsHostInfrastructureMemoryEntriesQueryKey,
  settingsHostInfrastructureMemoryOverviewQueryKey,
  settingsHostInfrastructureMemorySearchQueryKey,
  settingsHostInfrastructureMemoryStatsOverviewQueryKey,
  settingsHostInfrastructureMemoryTreeQueryKey,
  type SettingsHostInfrastructureMemoryEntry,
  type SettingsHostInfrastructureMemoryEntryValue,
  type SettingsHostInfrastructureMemoryTreeNode
} from '../../api/host-infrastructure';
import { i18nText } from '../../../../shared/i18n/text';
import { MemoryStatsOverviewPane } from './HostInfrastructureMemoryStatsOverviewPane';
import {
  formatBytes,
  formatInspectionPath,
  formatTtl,
  formatUnixTimestamp,
  formatUpdatedAt,
  resolveCanReveal
} from './host-infrastructure-memory-format';
import {
  collectTreeSearchItems,
  findTreeKeyByPath,
  toTreeData
} from './host-infrastructure-memory-tree';

const MEMORY_STATS_TAB_KEY = 'stats-overview';

export function HostInfrastructureMemoryObservationPanel({
  canManage
}: {
  canManage: boolean;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [activeTabKey, setActiveTabKey] = useState(MEMORY_STATS_TAB_KEY);
  const [selectedInspectionPath, setSelectedInspectionPath] = useState<
    string[] | null
  >(null);
  const [entryCursor, setEntryCursor] = useState<string | null>(null);
  const [cursorHistory, setCursorHistory] = useState<string[]>([]);
  const [searchText, setSearchText] = useState('');
  const [submittedSearch, setSubmittedSearch] = useState('');
  const [treeSearchText, setTreeSearchText] = useState('');
  const [treeExpandedKeys, setTreeExpandedKeys] = useState<Key[]>([]);
  const [treeAutoExpandParent, setTreeAutoExpandParent] = useState(true);
  const [loadedTreeChildren, setLoadedTreeChildren] = useState<
    Record<string, SettingsHostInfrastructureMemoryTreeNode[]>
  >({});
  const [metadataEntry, setMetadataEntry] =
    useState<SettingsHostInfrastructureMemoryEntry | null>(null);
  const [revealedEntry, setRevealedEntry] =
    useState<SettingsHostInfrastructureMemoryEntryValue | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(false);
  const queryClient = useQueryClient();

  const [sidebarWidth, setSidebarWidth] = useState(320);
  const dragInfoRef = useRef<{
    isDragging: boolean;
    startX: number;
    startWidth: number;
  } | null>(null);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!dragInfoRef.current || !dragInfoRef.current.isDragging) {
      return;
    }
    const deltaX = e.clientX - dragInfoRef.current.startX;
    const newWidth = Math.max(
      260,
      Math.min(600, dragInfoRef.current.startWidth + deltaX)
    );
    setSidebarWidth(newWidth);
  }, []);

  const handleMouseUp = useCallback(() => {
    if (dragInfoRef.current) {
      dragInfoRef.current.isDragging = false;
    }
    document.removeEventListener('mousemove', handleMouseMove);
    document.removeEventListener('mouseup', handleMouseUp);
  }, [handleMouseMove]);

  const startResizing = (e: React.MouseEvent) => {
    e.preventDefault();
    dragInfoRef.current = {
      isDragging: true,
      startX: e.clientX,
      startWidth: sidebarWidth
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  useEffect(() => {
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [handleMouseMove, handleMouseUp]);

  const overviewQuery = useQuery({
    queryKey: settingsHostInfrastructureMemoryOverviewQueryKey,
    queryFn: fetchSettingsHostInfrastructureMemoryOverview
  });
  const contracts = overviewQuery.data?.contracts ?? [];
  const resolvedActiveContractCode =
    activeTabKey !== MEMORY_STATS_TAB_KEY &&
    contracts.some((contract) => contract.contract_code === activeTabKey)
      ? activeTabKey
      : null;
  const activeContract = contracts.find(
    (contract) => contract.contract_code === resolvedActiveContractCode
  );
  const resolvedActiveTabKey =
    resolvedActiveContractCode || activeTabKey === MEMORY_STATS_TAB_KEY
      ? activeTabKey
      : MEMORY_STATS_TAB_KEY;
  const pageSize = activeContract?.capabilities.default_page_size ?? 50;
  const canListEntries = Boolean(
    activeContract?.supported && activeContract.capabilities.list_entries
  );
  const canListTree = Boolean(
    activeContract?.supported && activeContract.capabilities.list_tree
  );
  const canSearchEntries = Boolean(
    activeContract?.supported && activeContract.capabilities.search_entries
  );
  const entryRequest = selectedInspectionPath
    ? {
        inspection_path: selectedInspectionPath,
        cursor: entryCursor,
        limit: pageSize
      }
    : undefined;
  const entriesQuery = useQuery({
    queryKey: submittedSearch
      ? canSearchEntries
        ? settingsHostInfrastructureMemorySearchQueryKey(
            resolvedActiveContractCode,
            entryRequest
              ? { ...entryRequest, q: submittedSearch }
              : { q: submittedSearch }
          )
        : settingsHostInfrastructureMemoryEntriesQueryKey(
            resolvedActiveContractCode,
            entryRequest
          )
      : settingsHostInfrastructureMemoryEntriesQueryKey(
          resolvedActiveContractCode,
          entryRequest
        ),
    queryFn: () => {
      if (!resolvedActiveContractCode || !entryRequest) {
        return Promise.resolve(null);
      }
      if (submittedSearch && canSearchEntries) {
        return searchSettingsHostInfrastructureMemoryEntries(
          resolvedActiveContractCode,
          { ...entryRequest, q: submittedSearch }
        );
      }
      return fetchSettingsHostInfrastructureMemoryEntries(
        resolvedActiveContractCode,
        entryRequest
      );
    },
    enabled: Boolean(
      resolvedActiveContractCode && canListEntries && entryRequest
    )
  });
  const statsOverviewQuery = useQuery({
    queryKey: settingsHostInfrastructureMemoryStatsOverviewQueryKey,
    queryFn: fetchSettingsHostInfrastructureMemoryStatsOverview,
    enabled: Boolean(contracts.length)
  });
  const rootTreeQuery = useQuery({
    queryKey: settingsHostInfrastructureMemoryTreeQueryKey(
      resolvedActiveContractCode,
      {
        inspection_path: [],
        limit: pageSize
      }
    ),
    queryFn: () =>
      resolvedActiveContractCode
        ? fetchSettingsHostInfrastructureMemoryTree(
            resolvedActiveContractCode,
            {
              inspection_path: [],
              limit: pageSize
            }
          )
        : Promise.resolve(null),
    enabled: Boolean(resolvedActiveContractCode && canListTree)
  });
  const entries = entriesQuery.data?.entries ?? [];
  const canReveal = resolveCanReveal(
    canManage,
    overviewQuery.data?.can_manage,
    activeContract
  );

  useEffect(() => {
    setLoadedTreeChildren({});
    setSelectedInspectionPath(null);
    setEntryCursor(null);
    setCursorHistory([]);
    setSubmittedSearch('');
    setSearchText('');
    setTreeSearchText('');
    setTreeExpandedKeys([]);
    setTreeAutoExpandParent(true);
  }, [resolvedActiveContractCode]);

  const refreshMemoryQueries = useCallback(
    async (contractCode: string | null) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsHostInfrastructureMemoryOverviewQueryKey
        }),
        queryClient.invalidateQueries({
          queryKey: [
            'settings',
            'host-infrastructure',
            'memory',
            'contracts',
            contractCode
          ]
        })
      ]);
    },
    [queryClient]
  );

  useEffect(() => {
    if (!autoRefresh) return;
    const interval = setInterval(() => {
      void refreshMemoryQueries(resolvedActiveContractCode);
    }, 30000);
    return () => clearInterval(interval);
  }, [autoRefresh, refreshMemoryQueries, resolvedActiveContractCode]);

  const revealMutation = useMutation({
    mutationFn: async ({
      entry,
      revealMode
    }: {
      entry: SettingsHostInfrastructureMemoryEntry;
      revealMode: 'preview' | 'full';
    }) => {
      if (!csrfToken) {
        throw new Error('csrf_missing');
      }
      return revealSettingsHostInfrastructureMemoryEntry(
        entry.contract_code,
        entry.entry_ref,
        csrfToken,
        revealMode
      );
    },
    onSuccess: (value) => {
      setRevealedEntry(value);
    }
  });

  const rootNodes = useMemo(
    () => rootTreeQuery.data?.nodes ?? [],
    [rootTreeQuery.data?.nodes]
  );
  const treeData = useMemo(
    () => toTreeData(rootNodes, loadedTreeChildren, treeSearchText),
    [loadedTreeChildren, rootNodes, treeSearchText]
  );
  const treeSearchItems = useMemo(
    () => collectTreeSearchItems(treeData),
    [treeData]
  );
  const selectedTreeKey = findTreeKeyByPath(treeData, selectedInspectionPath);

  const updateTreeSearchText = (value: string) => {
    setTreeSearchText(value);
    const normalizedValue = value.trim().toLowerCase();
    if (!normalizedValue) {
      setTreeExpandedKeys([]);
      setTreeAutoExpandParent(false);
      return;
    }
    const matchedParentKeys = treeSearchItems
      .filter((item) => item.title.toLowerCase().includes(normalizedValue))
      .map((item) => item.parentKey)
      .filter((key): key is Key => key != null)
      .filter((key, index, keys) => keys.indexOf(key) === index);
    setTreeExpandedKeys(matchedParentKeys);
    setTreeAutoExpandParent(true);
  };

  const loadTreeChildren = async (treeNode: DataNode) => {
    if (!resolvedActiveContractCode) {
      return;
    }
    const node = treeNode as DataNode & {
      inspectionPath?: string[];
      isLeaf?: boolean;
    };
    if (node.isLeaf || !node.inspectionPath) {
      return;
    }
    const response = await queryClient.fetchQuery({
      queryKey: settingsHostInfrastructureMemoryTreeQueryKey(
        resolvedActiveContractCode,
        { inspection_path: node.inspectionPath, limit: pageSize }
      ),
      queryFn: () =>
        fetchSettingsHostInfrastructureMemoryTree(resolvedActiveContractCode, {
          inspection_path: node.inspectionPath,
          limit: pageSize
        })
    });
    setLoadedTreeChildren((current) => ({
      ...current,
      [String(node.key)]: response.nodes
    }));
  };

  const selectTab = (tabKey: string) => {
    setActiveTabKey(tabKey);
  };

  const selectInspectionPath = (path: string[]) => {
    setSelectedInspectionPath(path);
    setEntryCursor(null);
    setCursorHistory([]);
  };

  const entryColumns = useMemo<
    ColumnsType<SettingsHostInfrastructureMemoryEntry>
  >(
    () => [
      {
        title: i18nText('settings', 'auto.key'),
        dataIndex: 'key',
        key: 'key',
        width: 220,
        render: (key: string) => (
          <Tooltip title={key} placement="topLeft">
            <Typography.Text
              copyable
              ellipsis={{ tooltip: false }}
              style={{
                maxWidth: 160,
                display: 'inline-block',
                verticalAlign: 'middle'
              }}
              className="host-memory-panel__key"
            >
              {key}
            </Typography.Text>
          </Tooltip>
        )
      },
      {
        title: i18nText('settings', 'auto.group'),
        dataIndex: 'group_code',
        key: 'group_code',
        width: 160,
        render: (group: string) => (
          <Tooltip title={group} placement="topLeft">
            <Typography.Text
              ellipsis={{ tooltip: false }}
              style={{
                maxWidth: 120,
                display: 'inline-block',
                verticalAlign: 'middle'
              }}
            >
              {group}
            </Typography.Text>
          </Tooltip>
        )
      },
      {
        title: i18nText('settings', 'auto.kind'),
        dataIndex: 'entry_kind',
        key: 'entry_kind',
        width: 130
      },
      {
        title: i18nText('settings', 'auto.status'),
        dataIndex: 'status',
        key: 'status',
        width: 110,
        render: (status: string) => <Tag>{status}</Tag>
      },
      {
        title: i18nText('settings', 'auto.sensitive'),
        dataIndex: 'sensitive',
        key: 'sensitive',
        width: 110,
        render: (sensitive: boolean) => (
          <Tag color={sensitive ? 'red' : 'default'}>
            {sensitive
              ? i18nText('settings', 'auto.yes')
              : i18nText('settings', 'auto.no')}
          </Tag>
        )
      },
      {
        title: i18nText('settings', 'auto.ttl'),
        dataIndex: 'ttl_seconds',
        key: 'ttl_seconds',
        width: 120,
        render: (ttl: number | null) => formatTtl(ttl)
      },
      {
        title: i18nText('settings', 'auto.size'),
        dataIndex: 'value_size_bytes',
        key: 'value_size_bytes',
        width: 110,
        render: (size: number) => formatBytes(size)
      },
      {
        title: '',
        key: 'actions',
        width: canReveal ? 220 : 120,
        render: (_, entry) => (
          <Space size={4}>
            <Tooltip
              title={i18nText(
                'settings',
                'auto.check_meta_information_entry_read_actual_value'
              )}
            >
              <Button
                icon={<FileSearchOutlined />}
                onClick={() => setMetadataEntry(entry)}
                size="small"
              >
                {i18nText('settings', 'auto.metadata')}
              </Button>
            </Tooltip>
            {canReveal ? (
              <Tooltip
                title={i18nText(
                  'settings',
                  'auto.request_backend_read_real_value_preview_displayed_first_may_contain'
                )}
              >
                <Button
                  icon={<EyeOutlined />}
                  loading={revealMutation.isPending}
                  onClick={() => {
                    revealMutation.mutate({ entry, revealMode: 'preview' });
                  }}
                  size="small"
                >
                  {i18nText('settings', 'auto.reveal')}
                </Button>
              </Tooltip>
            ) : null}
          </Space>
        )
      }
    ],
    [canReveal, revealMutation]
  );

  if (overviewQuery.isError) {
    return (
      <Space orientation="vertical" size={16} className="host-memory-panel">
        <Alert
          type="error"
          showIcon
          title={i18nText('settings', 'auto.memory_watch_connection_failed')}
          description={i18nText(
            'settings',
            'auto.unable_read_host_infrastructure_memory_observation_api_server'
          )}
        />
        <Button
          icon={<ReloadOutlined />}
          onClick={() => overviewQuery.refetch()}
          loading={overviewQuery.isFetching}
        >
          {i18nText('settings', 'auto.refresh')}
        </Button>
      </Space>
    );
  }

  return (
    <Space orientation="vertical" size={16} className="host-memory-panel">
      <div className="host-memory-panel__toolbar">
        <Space size={[8, 8]} wrap>
          <Tag color="blue">
            {i18nText('settings', 'auto.contracts_count', {
              value1: contracts.length
            })}
          </Tag>
          <Tag>
            {i18nText('settings', 'auto.reveal_status', {
              value1: overviewQuery.data?.can_manage
                ? i18nText('settings', 'auto.available')
                : i18nText('settings', 'auto.off')
            })}
          </Tag>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.recently_refreshed_alt')}
            {formatUpdatedAt(overviewQuery.dataUpdatedAt)}
          </Typography.Text>
        </Space>
        <Space size={12} align="center">
          <Space size={6} align="center">
            <Switch
              checked={autoRefresh}
              onChange={(checked) => setAutoRefresh(checked)}
              size="small"
            />
            <Typography.Text type="secondary" style={{ fontSize: 13 }}>
              {i18nText('settings', 'auto.auto_refresh_three_zero_s')}
            </Typography.Text>
          </Space>
          <Button
            icon={<ReloadOutlined />}
            onClick={() => {
              void refreshMemoryQueries(resolvedActiveContractCode);
            }}
            loading={
              overviewQuery.isFetching ||
              entriesQuery.isFetching ||
              rootTreeQuery.isFetching ||
              statsOverviewQuery.isFetching
            }
          >
            {i18nText('settings', 'auto.refresh')}
          </Button>
        </Space>
      </div>

      {resolvedActiveContractCode && !canReveal ? (
        <Alert
          type="info"
          showIcon
          title={i18nText('settings', 'auto.view_displays_metadata')}
          description={i18nText(
            'settings',
            'auto.reveal_value_requires_infrastructure_manage_permissions_reveal_value_capability_contract'
          )}
        />
      ) : null}

      {overviewQuery.isSuccess && !contracts.length ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText(
            'settings',
            'auto.currently_observable_memory_contract'
          )}
        />
      ) : null}

      {contracts.length ? (
        <Tabs
          activeKey={resolvedActiveTabKey}
          className="host-memory-panel__tabs"
          items={[
            {
              key: MEMORY_STATS_TAB_KEY,
              label: i18nText('settings', 'auto.statistics'),
              children: (
                <div className="host-memory-panel__tab-pane">
                  <MemoryStatsOverviewPane
                    data={statsOverviewQuery.data}
                    isError={statsOverviewQuery.isError}
                    isLoading={statsOverviewQuery.isLoading}
                  />
                </div>
              )
            },
            ...contracts.map((contract) => ({
              key: contract.contract_code,
              label: (
                <span className="host-memory-panel__tab-label">
                  <span>{contract.label}</span>
                </span>
              ),
              children:
                contract.contract_code === resolvedActiveContractCode ? (
                  <div className="host-memory-panel__tab-pane">
                    <Layout
                      className="host-memory-panel__content"
                      data-testid="host-memory-panel-content"
                    >
                      <Layout.Sider
                        className="host-memory-panel__tree"
                        data-testid="host-memory-panel-tree"
                        theme="light"
                        width={sidebarWidth}
                        style={{
                          width: sidebarWidth,
                          minWidth: sidebarWidth,
                          maxWidth: sidebarWidth,
                          flex: `0 0 ${sidebarWidth}px`
                        }}
                      >
                        {activeContract ? (
                          !activeContract.supported ||
                          !activeContract.capabilities.list_tree ? (
                            <Alert
                              type="warning"
                              showIcon
                              title={i18nText(
                                'settings',
                                'auto.contract_support_tree_inspection'
                              )}
                            />
                          ) : rootTreeQuery.isError ? (
                            <Alert
                              type="error"
                              showIcon
                              title={i18nText(
                                'settings',
                                'auto.memory_tree_loading_failed'
                              )}
                            />
                          ) : rootTreeQuery.isSuccess && !rootNodes.length ? (
                            <Empty
                              image={Empty.PRESENTED_IMAGE_SIMPLE}
                              description={i18nText(
                                'settings',
                                'auto.memory_node_yet'
                              )}
                            />
                          ) : (
                            <div
                              className="host-memory-panel__tree-panel"
                              style={{
                                height: '100%',
                                display: 'flex',
                                flexDirection: 'column',
                                gap: '8px',
                                width: '100%'
                              }}
                            >
                              <Input.Search
                                allowClear
                                aria-label={i18nText(
                                  'settings',
                                  'auto.search_memory_tree'
                                )}
                                placeholder={i18nText(
                                  'settings',
                                  'auto.search_memory_tree'
                                )}
                                value={treeSearchText}
                                onChange={(event) =>
                                  updateTreeSearchText(event.target.value)
                                }
                              />
                              <div
                                className="host-memory-panel__tree-body"
                                style={{ flex: '1 1 0%', overflow: 'auto' }}
                              >
                                <Tree
                                  blockNode
                                  switcherIcon={
                                    <span data-testid="host-memory-panel-tree-switcher" />
                                  }
                                  autoExpandParent={treeAutoExpandParent}
                                  expandedKeys={treeExpandedKeys}
                                  treeData={treeData}
                                  loadData={loadTreeChildren}
                                  selectedKeys={
                                    selectedTreeKey ? [selectedTreeKey] : []
                                  }
                                  onExpand={(keys) => {
                                    setTreeExpandedKeys(keys);
                                    setTreeAutoExpandParent(false);
                                  }}
                                  onSelect={(_, info) => {
                                    const node = info.node as DataNode & {
                                      inspectionPath?: string[];
                                    };
                                    if (node.inspectionPath) {
                                      selectInspectionPath(node.inspectionPath);
                                    }
                                  }}
                                />
                              </div>
                            </div>
                          )
                        ) : (
                          <Empty
                            image={Empty.PRESENTED_IMAGE_SIMPLE}
                            description={i18nText(
                              'settings',
                              'auto.select_memory_contract'
                            )}
                          />
                        )}
                      </Layout.Sider>

                      <div
                        className="host-memory-panel__resize-handle"
                        onMouseDown={startResizing}
                      />

                      <Layout.Content
                        className="host-memory-panel__entries"
                        data-testid="host-memory-panel-entries"
                      >
                        <div
                          style={{
                            height: '100%',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '12px',
                            width: '100%'
                          }}
                        >
                          <div className="host-memory-panel__entries-header">
                            <Space orientation="vertical" size={2}>
                              <Typography.Text strong>
                                {i18nText('settings', 'auto.entries')}
                              </Typography.Text>
                              <Typography.Text type="secondary">
                                {selectedInspectionPath
                                  ? formatInspectionPath(selectedInspectionPath)
                                  : i18nText(
                                      'settings',
                                      'auto.no_path_selected'
                                    )}
                              </Typography.Text>
                            </Space>
                            <Input.Search
                              allowClear
                              disabled={!canSearchEntries}
                              value={searchText}
                              onChange={(event) =>
                                setSearchText(event.target.value)
                              }
                              onSearch={(value) => {
                                if (!canSearchEntries) {
                                  return;
                                }
                                setSubmittedSearch(value.trim());
                                setEntryCursor(null);
                                setCursorHistory([]);
                              }}
                              size="small"
                              style={{ maxWidth: 240 }}
                            />
                          </div>

                          {!selectedInspectionPath ? (
                            <Empty
                              image={Empty.PRESENTED_IMAGE_SIMPLE}
                              description={i18nText(
                                'settings',
                                'auto.select_tree_node'
                              )}
                            />
                          ) : entriesQuery.isError ? (
                            <Alert
                              type="error"
                              showIcon
                              title={i18nText(
                                'settings',
                                'auto.memory_entry_connection_failed'
                              )}
                              description={i18nText(
                                'settings',
                                'auto.unable_read_entries_path'
                              )}
                            />
                          ) : entriesQuery.isSuccess && !entries.length ? (
                            <Empty
                              image={Empty.PRESENTED_IMAGE_SIMPLE}
                              description={i18nText(
                                'settings',
                                'auto.memory_entry_yet'
                              )}
                            />
                          ) : (
                            <>
                              <div
                                className="host-memory-panel__table-wrapper"
                                style={{ flex: '1 1 0%', overflow: 'auto' }}
                              >
                                <Table
                                  rowKey={(entry) => entry.entry_ref}
                                  columns={entryColumns}
                                  dataSource={entries}
                                  loading={
                                    entriesQuery.isLoading ||
                                    entriesQuery.isFetching
                                  }
                                  pagination={false}
                                  size="small"
                                />
                              </div>
                              <div className="host-memory-panel__entries-header">
                                <Typography.Text type="secondary">
                                  {entriesQuery.data
                                    ? `${formatBytes(
                                        entriesQuery.data.emitted_bytes
                                      )} ${i18nText('settings', 'auto.emitted')}`
                                    : null}
                                </Typography.Text>
                                <Space size={8}>
                                  <Button
                                    size="small"
                                    disabled={!cursorHistory.length}
                                    onClick={() => {
                                      setCursorHistory((current) => {
                                        const previousCursor =
                                          current.at(-1) ?? null;
                                        const nextHistory = current.slice(
                                          0,
                                          -1
                                        );
                                        setEntryCursor(previousCursor || null);
                                        return nextHistory;
                                      });
                                    }}
                                  >
                                    {i18nText('settings', 'auto.previous_page')}
                                  </Button>
                                  <Button
                                    size="small"
                                    disabled={!entriesQuery.data?.next_cursor}
                                    onClick={() => {
                                      const nextCursor =
                                        entriesQuery.data?.next_cursor;
                                      if (!nextCursor) {
                                        return;
                                      }
                                      setCursorHistory((current) => [
                                        ...current,
                                        entryCursor ?? ''
                                      ]);
                                      setEntryCursor(nextCursor);
                                    }}
                                  >
                                    {i18nText('settings', 'auto.next_page')}
                                  </Button>
                                </Space>
                              </div>
                            </>
                          )}
                        </div>
                      </Layout.Content>
                    </Layout>
                  </div>
                ) : null
            }))
          ]}
          onChange={selectTab}
        />
      ) : null}

      <Drawer
        title={i18nText('settings', 'auto.entry_metadata')}
        size={640}
        open={Boolean(metadataEntry)}
        onClose={() => setMetadataEntry(null)}
        destroyOnHidden
      >
        {metadataEntry ? (
          <Space
            orientation="vertical"
            size={16}
            className="host-memory-panel__drawer"
          >
            <Descriptions column={1} size="small">
              <Descriptions.Item label={i18nText('settings', 'auto.contract')}>
                {metadataEntry.contract_code}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.group')}>
                {metadataEntry.group_code}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.key')}>
                {metadataEntry.key}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.entry_ref')}>
                {metadataEntry.entry_ref}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.path')}>
                {formatInspectionPath(metadataEntry.inspection_path)}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.owner')}>
                {metadataEntry.owner ?? i18nText('settings', 'auto.unknown')}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.created')}>
                {formatUnixTimestamp(metadataEntry.created_at_unix)}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.expires')}>
                {formatUnixTimestamp(metadataEntry.expires_at_unix)}
              </Descriptions.Item>
            </Descriptions>
            <JsonPreviewBlock
              title={i18nText('settings', 'auto.metadata')}
              value={metadataEntry.metadata}
              collapsible={false}
              height="320px"
              copySuccessMessage={i18nText(
                'settings',
                'auto.metadata_json_copied'
              )}
            />
          </Space>
        ) : null}
      </Drawer>

      <Drawer
        title={i18nText('settings', 'auto.entry_value')}
        size={640}
        open={Boolean(revealedEntry)}
        onClose={() => setRevealedEntry(null)}
        destroyOnHidden
      >
        {revealedEntry ? (
          <Space
            orientation="vertical"
            size={16}
            className="host-memory-panel__drawer"
          >
            <Descriptions column={1} size="small">
              <Descriptions.Item label={i18nText('settings', 'auto.contract')}>
                {revealedEntry.metadata.contract_code}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.group')}>
                {revealedEntry.metadata.group_code}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.key')}>
                {revealedEntry.metadata.key}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.entry_ref')}>
                {revealedEntry.metadata.entry_ref}
              </Descriptions.Item>
              <Descriptions.Item
                label={i18nText('settings', 'auto.value_state')}
              >
                {revealedEntry.value_state}
              </Descriptions.Item>
              <Descriptions.Item
                label={i18nText('settings', 'auto.reveal_mode')}
              >
                {revealedEntry.reveal_mode}
              </Descriptions.Item>
              <Descriptions.Item label={i18nText('settings', 'auto.size')}>
                {formatBytes(revealedEntry.metadata.value_size_bytes)}
              </Descriptions.Item>
            </Descriptions>
            {revealedEntry.value_state === 'available' ? (
              <JsonPreviewBlock
                title={i18nText('settings', 'auto.memory_value')}
                value={revealedEntry.value}
                collapsible={false}
                height="360px"
                copySuccessMessage={i18nText(
                  'settings',
                  'auto.memory_json_copied'
                )}
              />
            ) : revealedEntry.value_preview ? (
              <Space orientation="vertical" size={8}>
                <Alert
                  type="info"
                  showIcon
                  title={i18nText('settings', 'auto.preview')}
                  description={`${formatBytes(
                    revealedEntry.preview_size_bytes
                  )} ${i18nText('settings', 'auto.of')} ${formatBytes(revealedEntry.full_value_size_bytes)}`}
                />
                <JsonPreviewBlock
                  title={i18nText('settings', 'auto.memory_value_preview')}
                  value={revealedEntry.value_preview}
                  rawText={revealedEntry.value_preview}
                  collapsible={false}
                  height="320px"
                  copySuccessMessage={i18nText(
                    'settings',
                    'auto.copied_memory_preview_json'
                  )}
                />
              </Space>
            ) : (
              <Alert
                type="warning"
                showIcon
                title={i18nText('settings', 'auto.value_too_large')}
                description={`${formatBytes(
                  revealedEntry.full_value_size_bytes
                )} ${i18nText('settings', 'auto.exceeds_full_reveal_limit')}`}
              />
            )}
            {canReveal && revealedEntry.value_state === 'preview' ? (
              <Button
                icon={<EyeOutlined />}
                loading={revealMutation.isPending}
                onClick={() =>
                  revealMutation.mutate({
                    entry: revealedEntry.metadata,
                    revealMode: 'full'
                  })
                }
              >
                {i18nText('settings', 'auto.full_reveal')}
              </Button>
            ) : null}
          </Space>
        ) : null}
      </Drawer>
    </Space>
  );
}
