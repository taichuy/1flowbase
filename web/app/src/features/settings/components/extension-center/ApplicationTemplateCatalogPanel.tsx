import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import type {
  ApplicationTemplateCatalogItem,
  ApplicationTemplateReference
} from '@1flowbase/api-client';
import { useNavigate } from '@tanstack/react-router';
import { UploadOutlined } from '@ant-design/icons';
import { Alert, Button, Flex, Input, Tabs } from 'antd';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { ApplicationTemplateCatalogList } from './ApplicationTemplateCatalogList';
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
  const can = useConsoleOperationAccess();
  const canViewCatalog = can('system_templates.catalog');
  const canInstall = can('system_templates.install');
  const canPreview = can('system_templates.preview');
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const catalog = useApplicationTemplateCatalog(canViewCatalog, { cursor, q });
  const { isFetching, hasNextPage, fetchNextPage } = catalog;
  const [searchText, setSearchText] = useState(q ?? '');
  const [selected, setSelected] = useState<{
    name: string;
    body: ApplicationTemplateReference;
  }>();
  const [file, setFile] = useState<File>();
  const fileInput = useRef<HTMLInputElement>(null);
  const loadingMore = useRef(new Set<string>());
  useEffect(() => setSearchText(q ?? ''), [q]);
  const loadMore = useCallback(async () => {
    const key = JSON.stringify([q, cursor]);
    if (loadingMore.current.has(key) || isFetching || !hasNextPage) return;
    loadingMore.current.add(key);
    try {
      await fetchNextPage({ cancelRefetch: false });
    } finally {
      loadingMore.current.delete(key);
    }
  }, [q, cursor, isFetching, hasNextPage, fetchNextPage]);
  const goTo = (nextCursor?: string, query = q) =>
    void navigate({
      to: '/settings/extension-center/$category',
      params: { category: 'application-templates' },
      search: { q: query || undefined, cursor: nextCursor }
    });
  const items = useMemo(
    () =>
      catalog.data?.pages.flatMap((page) => page.application_templates) ?? [],
    [catalog.data]
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16} className="application-template-catalog">
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
        {!canViewCatalog ? (
          <Alert type="warning" showIcon title={t('access_denied')} />
        ) : (
          <>
            {catalog.isError && (
              <Alert
                type="error"
                showIcon
                title={t('catalog_load_failed')}
                action={
                  <Button
                    onClick={() =>
                      void (catalog.isFetchNextPageError
                        ? loadMore()
                        : catalog.refetch())
                    }
                  >
                    {t('catalog_retry')}
                  </Button>
                }
              />
            )}
            <Flex
              justify="flex-end"
              gap={8}
              wrap
              className="application-template-catalog__toolbar"
            >
              <input
                ref={fileInput}
                type="file"
                hidden
                accept=".json,.zip,application/json,application/zip"
                aria-label={t('upload')}
                disabled={!csrfToken || !canPreview || !canInstall}
                onChange={(event) => {
                  const selectedFile = event.target.files?.[0];
                  event.target.value = '';
                  if (selectedFile && csrfToken && canPreview && canInstall) {
                    setSelected(undefined);
                    setFile(selectedFile);
                  }
                }}
              />
              <Button
                icon={<UploadOutlined />}
                aria-label={t('upload')}
                disabled={!csrfToken || !canPreview || !canInstall}
                onClick={() => fileInput.current?.click()}
              >
                {t('upload')}
              </Button>
              <Input.Search
                allowClear
                aria-label={t('catalog_search')}
                placeholder={t('catalog_search')}
                className="application-template-catalog__search"
                value={searchText}
                onChange={(event) => setSearchText(event.target.value)}
                onClear={() => goTo(undefined, '')}
                onSearch={(value) => goTo(undefined, value.trim())}
              />
            </Flex>
            <ApplicationTemplateCatalogList
              key={JSON.stringify([q, cursor])}
              items={items}
              total={catalog.data?.pages[0]?.total ?? 0}
              loading={catalog.isFetching}
              failed={catalog.isError}
              hasMore={catalog.hasNextPage}
              onLoadMore={loadMore}
              canInstall={canInstall && Boolean(csrfToken)}
              onInstall={(item: ApplicationTemplateCatalogItem) => {
                setFile(undefined);
                setSelected({
                  name: item.name,
                  body: {
                    catalog_id: item.catalog_id,
                    release_version: item.release_version
                  }
                });
              }}
            />
          </>
        )}
      </Flex>
      {(selected || file) && canViewCatalog && (
        <SystemTemplateDialog
          mode="import"
          template={selected}
          file={file}
          canInstall={canInstall && Boolean(csrfToken)}
          onClose={() => {
            setSelected(undefined);
            setFile(undefined);
          }}
        />
      )}
    </SettingsSectionSurface>
  );
}
