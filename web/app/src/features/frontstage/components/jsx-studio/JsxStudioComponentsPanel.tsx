import type { ConsoleFrontstageComponentCapabilitySummary } from '@1flowbase/api-client';
import { Alert, Button, Empty, Input, Space, Table, Typography, message } from 'antd';
import { useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { fetchFrontstageComponentCapability } from '../../api/component-capabilities';
import { useFrontstageComponentCapabilities } from '../../hooks/use-frontstage-component-capabilities';
import type { FrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import type { FrontstageJsxInsertion } from '../../lib/jsx-studio/source-insertion';

const PAGE_SIZE = 10;

export function JsxStudioComponentsPanel({
  componentCatalogQuery,
  onInsertCode,
  workspaceId
}: {
  componentCatalogQuery: FrontstageJsxEditorProjection['componentCatalogQuery'];
  onInsertCode: (insertion: FrontstageJsxInsertion) => void;
  workspaceId: string;
}) {
  const [query, setQuery] = useState('');
  const [offset, setOffset] = useState(0);
  const [copyingId, setCopyingId] = useState<string | null>(null);
  const componentPage = useFrontstageComponentCapabilities(
    workspaceId,
    {
      ...componentCatalogQuery,
      query: query || undefined,
      offset,
      limit: PAGE_SIZE
    },
    componentCatalogQuery !== null
  );

  const copyApi = async (componentId: string) => {
    setCopyingId(componentId);
    try {
      const component = await fetchFrontstageComponentCapability(
        workspaceId,
        componentId
      );
      await copyTextToClipboard(component.api_documentation);
      message.success(i18nText('frontstage', 'auto.component_api_copied'));
    } catch {
      message.warning(
        i18nText('frontstage', 'auto.copy_component_api_failed')
      );
    } finally {
      setCopyingId(null);
    }
  };

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <section className="frontstage-jsx-studio__resource-section">
        <Typography.Title level={5}>
          {i18nText('frontstage', 'auto.components')}
        </Typography.Title>
        <Typography.Paragraph type="secondary">
          {i18nText('frontstage', 'auto.components_description')}
        </Typography.Paragraph>
        <Input.Search
          allowClear
          aria-label={i18nText('frontstage', 'auto.search_components')}
          placeholder={i18nText('frontstage', 'auto.search_components')}
          onSearch={(value) => {
            setQuery(value.trim());
            setOffset(0);
          }}
        />
        {componentPage.error ? (
          <Alert
            type="error"
            showIcon
            message={i18nText(
              'frontstage',
              'auto.component_catalog_load_failed'
            )}
          />
        ) : null}
        <Table<ConsoleFrontstageComponentCapabilitySummary>
          rowKey="component_id"
          size="small"
          loading={componentPage.loading}
          dataSource={componentPage.data.items}
          columns={[
            {
              title: i18nText('frontstage', 'auto.components'),
              dataIndex: 'export_name',
              width: 92,
              render: (value: string) => (
                <Typography.Text code>{value}</Typography.Text>
              )
            },
            {
              title: i18nText('frontstage', 'auto.description'),
              dataIndex: 'description',
              render: (value: string) => (
                <Typography.Paragraph
                  style={{ marginBottom: 0 }}
                  ellipsis={{ rows: 2, tooltip: value }}
                >
                  {value}
                </Typography.Paragraph>
              )
            },
            {
              title: i18nText('frontstage', 'auto.operation'),
              key: 'actions',
              width: 126,
              render: (_, component) => (
                <Space size={2}>
                  <Button
                    type="link"
                    size="small"
                    onClick={() =>
                      onInsertCode({
                        kind: 'component',
                        name: component.export_name,
                        moduleSource: component.module_source,
                        source: component.insert_snippet
                      })
                    }
                  >
                    {i18nText('frontstage', 'auto.insert_component')}
                  </Button>
                  <Button
                    type="link"
                    size="small"
                    loading={copyingId === component.component_id}
                    onClick={() => void copyApi(component.component_id)}
                  >
                    {i18nText('frontstage', 'auto.copy_api')}
                  </Button>
                </Space>
              )
            }
          ]}
          locale={{
            emptyText: (
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description={i18nText(
                  'frontstage',
                  'auto.no_available_components'
                )}
              />
            )
          }}
          pagination={{
            current: Math.floor(offset / PAGE_SIZE) + 1,
            pageSize: PAGE_SIZE,
            total: componentPage.data.total,
            showSizeChanger: false,
            size: 'small',
            onChange: (page) => setOffset((page - 1) * PAGE_SIZE)
          }}
        />
      </section>
    </div>
  );
}
