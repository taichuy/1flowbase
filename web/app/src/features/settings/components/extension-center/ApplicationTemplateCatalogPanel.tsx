import type {
  ApplicationTemplateCatalogItem,
  ApplicationTemplateReference
} from '@1flowbase/api-client';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Empty, Flex, Input, Tabs, Typography } from 'antd';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import {
  DataTable,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import { useApplicationTemplateCatalog } from '../../api/system-templates/useSystemTemplates';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { SystemTemplateDialog } from '../system-templates/SystemTemplateDialog';

export function ApplicationTemplateCatalogPanel({
  cursor,
  q
}: {
  cursor?: string;
  q?: string;
}) {
  const navigate = useNavigate();
  const { t } = useTranslation('settingsSystemTemplates');
  const { t: settingsT } = useTranslation('settings');
  const canManage = useAuthStore(
    (state) =>
      state.actor?.effective_display_role === 'root' ||
      (state.me?.permissions.includes(
        'settings_feature.access.system.backups'
      ) ??
        false)
  );
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const catalog = useApplicationTemplateCatalog(canManage, { cursor, q });
  const [searchText, setSearchText] = useState(q ?? '');
  const [selected, setSelected] = useState<{
    name: string;
    body: ApplicationTemplateReference;
  }>();
  const previousPages = useRef(
    new Map<string, { cursor?: string; page: number }>()
  );
  useEffect(() => {
    setSearchText(q ?? '');
    previousPages.current.clear();
  }, [q]);
  const currentPage = cursor
    ? (previousPages.current.get(cursor)?.page ?? 2)
    : 1;
  const goTo = (nextCursor?: string, query = q) =>
    void navigate({
      to: '/settings/extension-center/$category',
      params: { category: 'application-templates' },
      search: { q: query || undefined, cursor: nextCursor }
    });
  const columns = useMemo<
    Array<DataTableColumn<ApplicationTemplateCatalogItem>>
  >(
    () => [
      {
        title: t('template_name'),
        dataIndex: 'name',
        key: 'name',
        width: 220,
        ellipsis: true
      },
      {
        title: t('description'),
        dataIndex: 'description',
        key: 'description',
        width: 320,
        sizing: 'fill',
        ellipsis: true
      },
      {
        title: t('version'),
        dataIndex: 'release_version',
        key: 'release_version',
        width: 100
      },
      {
        title: t('installed_version'),
        dataIndex: 'installed_release_version',
        key: 'installed_release_version',
        width: 150,
        render: (_, item) =>
          item.installed_release_version ?? t('not_installed')
      },
      {
        title: t('source'),
        dataIndex: 'source',
        key: 'source',
        width: 120,
        render: (_, item) =>
          item.source === 'builtin' ? t('source_builtin') : t('source_official')
      },
      {
        title: t('action'),
        key: 'action',
        width: 180,
        render: (_, item) => (
          <Button
            disabled={!csrfToken}
            onClick={() =>
              setSelected({
                name: item.name,
                body: {
                  catalog_id: item.catalog_id,
                  release_version: item.release_version
                }
              })
            }
          >
            {t('preview_action')}
          </Button>
        )
      }
    ],
    [t, csrfToken]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.application_templates'
  });

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey="application-templates"
          onChange={(category) =>
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category },
              search: { q: undefined, cursor: undefined }
            })
          }
          items={[
            { key: 'installed', label: settingsT('auto.installed_extensions') },
            {
              key: 'application-templates',
              label: settingsT('auto.application_templates')
            },
            { key: 'agent-flow', label: 'agent-flow' },
            { key: 'capability-plugins', label: 'capability-plugins' },
            { key: 'host-extensions', label: 'host-extensions' },
            { key: 'i18n', label: 'i18n' },
            { key: 'mcp', label: 'mcp' },
            { key: 'runtime-extensions', label: 'runtime-extensions' },
            { key: 'ui-components', label: settingsT('auto.ui_components') },
            {
              key: 'model-pricing',
              label: settingsT('auto.billing_vendor_model_pricing')
            }
          ]}
        />
        <Typography.Title level={4}>{t('catalog_title')}</Typography.Title>
        {!canManage ? (
          <Alert type="warning" showIcon title={t('access_denied')} />
        ) : (
          <>
            {catalog.isError && (
              <Alert
                type="error"
                showIcon
                title={t('catalog_load_failed')}
                action={
                  <Button onClick={() => void catalog.refetch()}>
                    {t('catalog_retry')}
                  </Button>
                }
              />
            )}
            <DataTable<ApplicationTemplateCatalogItem>
              rowKey="catalog_id"
              columns={columns}
              configuration={tableConfiguration}
              loading={catalog.isFetching}
              dataSource={catalog.data?.application_templates ?? []}
              emptyText={<Empty description={t('catalog_empty')} />}
              toolbar={
                <Flex justify="flex-end" gap={8} wrap>
                  <Input.Search
                    allowClear
                    aria-label={t('catalog_search')}
                    placeholder={t('catalog_search')}
                    style={{ width: 240 }}
                    value={searchText}
                    onChange={(event) => setSearchText(event.target.value)}
                    onClear={() => goTo(undefined, '')}
                    onSearch={(value) => goTo(undefined, value.trim())}
                  />
                </Flex>
              }
              cursorPagination={{
                currentPage,
                hasPreviousPage: Boolean(cursor) && !catalog.isFetching,
                hasNextPage:
                  Boolean(catalog.data?.next_cursor) && !catalog.isFetching,
                previousLabel: settingsT('auto.previous_page'),
                nextLabel: settingsT('auto.next_page'),
                total: catalog.data?.total ?? 0,
                onPreviousPage: () =>
                  goTo(
                    cursor
                      ? previousPages.current.get(cursor)?.cursor
                      : undefined
                  ),
                onNextPage: () => {
                  const nextCursor = catalog.data?.next_cursor;
                  if (!nextCursor) return;
                  previousPages.current.set(nextCursor, {
                    cursor,
                    page: currentPage + 1
                  });
                  goTo(nextCursor);
                }
              }}
            />
          </>
        )}
      </Flex>
      {selected && canManage && (
        <SystemTemplateDialog
          mode="import"
          template={selected}
          canInstall={canManage && Boolean(csrfToken)}
          onClose={() => setSelected(undefined)}
        />
      )}
    </SettingsSectionSurface>
  );
}
