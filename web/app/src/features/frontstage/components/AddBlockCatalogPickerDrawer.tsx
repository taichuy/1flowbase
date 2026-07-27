import {
  Alert,
  Button,
  Drawer,
  Empty,
  List,
  Space,
  Tag,
  Typography
} from 'antd';
import type { FC } from 'react';

import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import { i18nText } from '../../../shared/i18n/text';

export interface AddBlockCatalogPickerDrawerProps {
  open: boolean;
  items: NormalizedFrontstageBlockCatalogEntry[];
  loading?: boolean;
  catalogError?: string | null;
  creationError?: string | null;
  saving?: boolean;
  onSelect: (entry: NormalizedFrontstageBlockCatalogEntry) => void;
  onClose: () => void;
}

export const AddBlockCatalogPickerDrawer: FC<
  AddBlockCatalogPickerDrawerProps
> = ({
  open,
  items,
  loading,
  catalogError,
  creationError,
  saving,
  onSelect,
  onClose
}) => {
  const isBusy = Boolean(loading || saving);

  return (
    <Drawer
      open={open}
      onClose={onClose}
      placement="right"
      title={i18nText('frontstage', 'auto.add_block')}
      width="min(520px, 100vw)"
    >
      <Space direction="vertical" size={12} style={{ width: '100%' }}>
        {catalogError ? (
          <Alert
            message={i18nText('frontstage', 'auto.block_catalog_load_failed')}
            description={catalogError}
            type="error"
            showIcon
          />
        ) : null}
        {creationError ? (
          <Alert
            message={i18nText('frontstage', 'auto.block_save_failed')}
            description={creationError}
            type="error"
            showIcon
          />
        ) : null}

        {items.length === 0 && !loading ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={
              <Typography.Text type="secondary">
                {i18nText(
                  'frontstage',
                  'auto.no_available_block_catalog_entries'
                )}
              </Typography.Text>
            }
          />
        ) : (
          <List
            loading={loading}
            dataSource={items}
            rowKey={(entry) =>
              `${entry.providerCode}:${entry.installationId}:${entry.contributionCode}`
            }
            renderItem={(entry) => {
              const hasCodeTemplate = Boolean(entry.codeCapabilities?.template);

              return (
                <List.Item
                  actions={[
                    <Button
                      key="select"
                      aria-label={i18nText('frontstage', 'auto.select')}
                      type="primary"
                      size="small"
                      disabled={isBusy || !hasCodeTemplate}
                      loading={saving}
                      onClick={() => onSelect(entry)}
                    >
                      {i18nText('frontstage', 'auto.select')}
                    </Button>
                  ]}
                >
                  <List.Item.Meta
                    title={entry.title}
                    description={
                      <Space
                        direction="vertical"
                        size={6}
                        style={{ width: '100%' }}
                      >
                        <Space size={6} wrap>
                          <Tag>{entry.runtimeKind}</Tag>
                          <Typography.Text type="secondary">
                            {entry.providerCode}
                          </Typography.Text>
                          <Typography.Text type="secondary">
                            {entry.contributionCode}
                          </Typography.Text>
                        </Space>
                        {!hasCodeTemplate ? (
                          <Alert
                            message={i18nText(
                              'frontstage',
                              'auto.catalog_entry_missing_code_template'
                            )}
                            type="error"
                            showIcon
                          />
                        ) : null}
                      </Space>
                    }
                  />
                </List.Item>
              );
            }}
          />
        )}
      </Space>
    </Drawer>
  );
};
