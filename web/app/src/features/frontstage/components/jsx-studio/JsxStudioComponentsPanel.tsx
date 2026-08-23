import type { ConsoleFrontstageComponent } from '@1flowbase/api-client';
import {
  Alert,
  Button,
  Empty,
  Input,
  Space,
  Table,
  Typography,
  App
} from 'antd';
import { useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { useFrontstageComponents } from '../../hooks/use-frontstage-components';
import type { FrontstageJsxInsertion } from '../../lib/jsx-studio/source-insertion';

const PAGE_SIZE = 10;

export function JsxStudioComponentsPanel({
  onInsertCode,
  workspaceId
}: {
  onInsertCode: (insertion: FrontstageJsxInsertion) => void;
  workspaceId: string;
}) {
  const { message } = App.useApp();
  const [query, setQuery] = useState('');
  const [offset, setOffset] = useState(0);
  const [copyingId, setCopyingId] = useState<string | null>(null);
  const [insertingId, setInsertingId] = useState<string | null>(null);
  const componentPage = useFrontstageComponents(
    workspaceId,
    {
      query: query || undefined,
      offset,
      limit: PAGE_SIZE
    },
    true
  );

  const copyApi = async (component: ConsoleFrontstageComponent) => {
    setCopyingId(component.id);
    try {
      await copyTextToClipboard(
        `${component.import_code}\n\n${component.source_code}`
      );
      message.success(i18nText('frontstage', 'auto.component_api_copied'));
    } catch {
      message.warning(i18nText('frontstage', 'auto.copy_component_api_failed'));
    } finally {
      setCopyingId(null);
    }
  };

  const insertComponent = async (component: ConsoleFrontstageComponent) => {
    setInsertingId(component.id);
    try {
      onInsertCode({
        kind: 'component',
        importCode: component.import_code,
        source: component.source_code
      });
    } catch {
      message.warning(
        i18nText('frontstage', 'auto.component_catalog_load_failed')
      );
    } finally {
      setInsertingId(null);
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
            title={i18nText('frontstage', 'auto.component_catalog_load_failed')}
          />
        ) : null}
        <Table<ConsoleFrontstageComponent>
          rowKey="id"
          size="small"
          loading={componentPage.loading}
          dataSource={componentPage.data.items}
          columns={[
            {
              title: i18nText('frontstage', 'auto.components'),
              dataIndex: 'name',
              width: 92,
              render: (value: string) => (
                <Typography.Text>{value}</Typography.Text>
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
                    loading={insertingId === component.id}
                    onClick={() => void insertComponent(component)}
                  >
                    {i18nText('frontstage', 'auto.insert_component')}
                  </Button>
                  <Button
                    type="link"
                    size="small"
                    loading={copyingId === component.id}
                    onClick={() => void copyApi(component)}
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
