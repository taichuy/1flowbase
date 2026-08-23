import { useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Descriptions,
  Flex,
  Input,
  Table,
  Tag,
  Typography,
  type TableColumnsType
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import {
  downloadSettingsUiCatalogComponent,
  fetchSettingsUiCatalogPage,
  fetchSettingsUiCatalogUpdateStatus,
  searchSettingsUiCatalog,
  settingsUiComponentsQueryKey,
  syncSettingsUiCatalogGroup,
  type SettingsUiCatalogComponent
} from '../../api/ui-management';

type CatalogRow = Pick<
  SettingsUiCatalogComponent,
  | 'component_code'
  | 'name'
  | 'description'
  | 'source'
  | 'group'
  | 'upstream'
  | 'version'
  | 'keywords'
>;

const SEARCH_PAGE_SIZE = 20;

function tokenOrThrow(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

export function RemoteCatalogDrawer({
  open,
  canManage,
  onClose
}: {
  open: boolean;
  canManage: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation('settingsUiManagement');
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [page, setPage] = useState(1);
  const [searchDraft, setSearchDraft] = useState('');
  const [search, setSearch] = useState('');
  const browsing = search.length === 0;
  const catalogPage = useQuery({
    queryKey: ['settings', 'ui-management', 'catalog', 'page', page],
    queryFn: () => fetchSettingsUiCatalogPage(page),
    enabled: open && browsing
  });
  const searchResult = useQuery({
    queryKey: ['settings', 'ui-management', 'catalog', 'search', search, page],
    queryFn: () => searchSettingsUiCatalog(search, page, SEARCH_PAGE_SIZE),
    enabled: open && !browsing
  });
  const updateStatus = useQuery({
    queryKey: ['settings', 'ui-management', 'catalog', 'update-status'],
    queryFn: () => fetchSettingsUiCatalogUpdateStatus(),
    enabled: open
  });
  const download = useMutation({
    mutationFn: (componentCode: string) =>
      downloadSettingsUiCatalogComponent(
        componentCode,
        tokenOrThrow(csrfToken)
      ),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsUiComponentsQueryKey
        }),
        updateStatus.refetch()
      ]);
      message.success(t('catalog_downloaded'));
    }
  });
  const syncGroup = useMutation({
    mutationFn: ({ source, group }: { source: string; group: string }) =>
      syncSettingsUiCatalogGroup(source, group, tokenOrThrow(csrfToken)),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsUiComponentsQueryKey
        }),
        updateStatus.refetch()
      ]);
      message.success(t('catalog_group_synced'));
    }
  });
  const rows: CatalogRow[] = browsing
    ? (catalogPage.data?.records ?? [])
    : (searchResult.data?.entries ?? []);
  const total = browsing
    ? (catalogPage.data?.total_components ?? 0)
    : (searchResult.data?.total_entries ?? 0);
  const pageSize = browsing
    ? (catalogPage.data?.page_size ?? SEARCH_PAGE_SIZE)
    : SEARCH_PAGE_SIZE;
  const failed =
    catalogPage.isError ||
    searchResult.isError ||
    updateStatus.isError ||
    download.isError ||
    syncGroup.isError;
  const columns = useMemo<TableColumnsType<CatalogRow>>(
    () => [
      { title: t('name'), dataIndex: 'name', key: 'name', width: 160 },
      {
        title: t('component_code'),
        dataIndex: 'component_code',
        key: 'component_code'
      },
      { title: t('group'), dataIndex: 'group', key: 'group', width: 150 },
      { title: t('version'), dataIndex: 'version', key: 'version', width: 100 },
      {
        title: t('actions'),
        key: 'actions',
        width: 120,
        render: (_, record) =>
          canManage ? (
            <Button
              type="link"
              size="small"
              loading={
                download.isPending &&
                download.variables === record.component_code
              }
              onClick={() => download.mutate(record.component_code)}
            >
              {t('catalog_download')}
            </Button>
          ) : null
      }
    ],
    [canManage, download, t]
  );

  return (
    <ResizableDrawer
      ariaLabel={t('remote_catalog')}
      defaultWidth={840}
      open={open}
      resizeLabel={t('resize_catalog_drawer')}
      title={t('remote_catalog')}
      onClose={onClose}
      extra={
        <Button
          onClick={() => {
            void updateStatus.refetch();
            if (browsing) void catalogPage.refetch();
            else void searchResult.refetch();
          }}
        >
          {t('component_refresh')}
        </Button>
      }
    >
      <Flex vertical gap={16} style={{ width: '100%' }}>
        {failed ? (
          <Alert type="error" showIcon message={t('catalog_request_failed')} />
        ) : null}
        <Flex align="center" gap={8} wrap>
          <Input.Search
            allowClear
            aria-label={t('catalog_search')}
            value={searchDraft}
            placeholder={t('catalog_search_placeholder')}
            onChange={(event) => setSearchDraft(event.target.value)}
            onSearch={(value) => {
              setSearch(value.trim());
              setPage(1);
            }}
          />
        </Flex>
        <Flex align="center" justify="space-between" gap={8} wrap>
          <Typography.Text>
            {updateStatus.data
              ? t('catalog_version_status', {
                  version: updateStatus.data.catalog_version,
                  count: updateStatus.data.groups.length
                })
              : t('catalog_status_loading')}
          </Typography.Text>
          {updateStatus.data?.update_available ? (
            <Tag color="processing">{t('catalog_update_available')}</Tag>
          ) : null}
        </Flex>
        {updateStatus.data ? (
          <Descriptions
            size="small"
            bordered
            column={1}
            items={updateStatus.data.groups.map((item) => ({
              key: `${item.source}/${item.group}`,
              label: `${item.source} / ${item.group}`,
              children: (
                <Flex align="center" justify="space-between" gap={8} wrap>
                  <Typography.Text type="secondary">
                    {t('catalog_group_status', {
                      total: item.remote_records,
                      updates: item.new_or_updated_records,
                      removals: item.removed_records
                    })}
                  </Typography.Text>
                  {canManage ? (
                    <Button
                      type="link"
                      size="small"
                      loading={
                        syncGroup.isPending &&
                        syncGroup.variables?.source === item.source &&
                        syncGroup.variables?.group === item.group
                      }
                      onClick={() =>
                        syncGroup.mutate({
                          source: item.source,
                          group: item.group
                        })
                      }
                    >
                      {t('catalog_sync_group')}
                    </Button>
                  ) : null}
                </Flex>
              )
            }))}
          />
        ) : null}
        <Table<CatalogRow>
          columns={columns}
          dataSource={rows}
          loading={catalogPage.isLoading || searchResult.isLoading}
          rowKey="component_code"
          pagination={{
            current: page,
            pageSize,
            total,
            showSizeChanger: false,
            onChange: setPage
          }}
          scroll={{ x: 760 }}
        />
      </Flex>
    </ResizableDrawer>
  );
}
