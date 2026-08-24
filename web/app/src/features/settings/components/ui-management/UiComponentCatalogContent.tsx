import { useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Flex,
  Input,
  Table,
  type TableColumnsType
} from 'antd';
import { useTranslation } from 'react-i18next';

import { useAuthStore } from '../../../../state/auth-store';
import {
  downloadSettingsUiCatalogComponent,
  fetchSettingsUiCatalogPage,
  searchSettingsUiCatalog,
  settingsUiComponentsQueryKey,
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
  | 'local_version'
>;

const SEARCH_PAGE_SIZE = 20;

function tokenOrThrow(token: string | null): string {
  if (!token) throw new Error('missing csrf token');
  return token;
}

export function UiComponentCatalogContent({
  canManage
}: {
  canManage: boolean;
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
    enabled: browsing
  });
  const searchResult = useQuery({
    queryKey: ['settings', 'ui-management', 'catalog', 'search', search, page],
    queryFn: () => searchSettingsUiCatalog(search, page, SEARCH_PAGE_SIZE),
    enabled: !browsing
  });
  const download = useMutation({
    mutationFn: (record: CatalogRow) =>
      downloadSettingsUiCatalogComponent(
        record.component_code,
        tokenOrThrow(csrfToken)
      ),
    onSuccess: async (_, record) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: settingsUiComponentsQueryKey
        }),
        browsing ? catalogPage.refetch() : searchResult.refetch()
      ]);
      message.success(
        t(record.local_version ? 'catalog_updated' : 'catalog_downloaded')
      );
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
    catalogPage.isError || searchResult.isError || download.isError;
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
                download.variables?.component_code === record.component_code
              }
              onClick={() => download.mutate(record)}
            >
              {t(record.local_version ? 'catalog_update' : 'catalog_download')}
            </Button>
          ) : null
      }
    ],
    [canManage, download, t]
  );

  return (
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
          style={{ flex: '1 1 320px' }}
          onChange={(event) => setSearchDraft(event.target.value)}
          onSearch={(value) => {
            setSearch(value.trim());
            setPage(1);
          }}
        />
        <Button
          onClick={() => {
            if (browsing) void catalogPage.refetch();
            else void searchResult.refetch();
          }}
        >
          {t('component_refresh')}
        </Button>
      </Flex>
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
  );
}
