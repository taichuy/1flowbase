import { useMemo } from 'react';
import type { ReactNode } from 'react';

import {
  Descriptions,
  Drawer,
  Empty,
  Grid,
  Space,
  Table,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';

import type {
  SettingsDataSourcePreview,
  SettingsDataSourceRemoteResource,
  SettingsRuntimeExtensionDataSource
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';
import '../../../../shared/ui/structured-list/structured-list.css';

const PREVIEW_FIELD_LIMIT = 20;
const PREVIEW_ROW_LIMIT = 20;
const PREVIEW_CELL_LIMIT = 400;
const PREVIEW_SCALAR_CHARACTER_LIMIT = 240;
const PREVIEW_FIELD_KEY_CHARACTER_LIMIT = 80;
const PREVIEW_NESTED_ENTRY_LIMIT = 8;
const PREVIEW_NESTED_DEPTH_LIMIT = 2;

function truncatePreviewText(text: string, characterLimit: number) {
  return {
    visibleText: text.slice(0, characterLimit),
    omittedCharacterCount: Math.max(0, text.length - characterLimit)
  };
}

function OmittedCharacterCount({ count }: { count: number }) {
  if (count === 0) {
    return null;
  }

  return (
    <Typography.Text type="secondary">
      {i18nText('settings', 'auto.preview_characters_omitted', {
        value1: count
      })}
    </Typography.Text>
  );
}

function PreviewFieldKey({ fieldKey }: { fieldKey: string }) {
  const { visibleText, omittedCharacterCount } = truncatePreviewText(
    fieldKey,
    PREVIEW_FIELD_KEY_CHARACTER_LIMIT
  );

  return (
    <Space orientation="vertical" size={0}>
      <Typography.Text>{visibleText}</Typography.Text>
      <OmittedCharacterCount count={omittedCharacterCount} />
    </Space>
  );
}

function PreviewScalarValue({ fullText }: { fullText: string }) {
  const { visibleText, omittedCharacterCount } = truncatePreviewText(
    fullText,
    PREVIEW_SCALAR_CHARACTER_LIMIT
  );

  return (
    <Space orientation="vertical" size={0}>
      <Typography.Text
        copyable={
          omittedCharacterCount > 0
            ? {
                text: fullText,
                tooltips: [
                  i18nText('settings', 'auto.copy_full_value'),
                  i18nText('settings', 'auto.full_value_copied')
                ]
              }
            : false
        }
      >
        {visibleText}
      </Typography.Text>
      <OmittedCharacterCount count={omittedCharacterCount} />
    </Space>
  );
}

function structuredEntryCount(value: unknown[] | Record<string, unknown>) {
  return Array.isArray(value) ? value.length : Object.keys(value).length;
}

function PreviewValue({
  value,
  depth = 0
}: {
  value: unknown;
  depth?: number;
}): ReactNode {
  if (value === null || value === undefined) {
    return <Typography.Text type="secondary">—</Typography.Text>;
  }
  if (typeof value !== 'object') {
    return <PreviewScalarValue fullText={String(value)} />;
  }

  const structuredValue = value as unknown[] | Record<string, unknown>;
  const entries = Array.isArray(structuredValue)
    ? structuredValue.map((entry, index) => [`[${index}]`, entry] as const)
    : Object.entries(structuredValue);
  if (depth >= PREVIEW_NESTED_DEPTH_LIMIT) {
    return (
      <Typography.Text type="secondary">
        {i18nText('settings', 'auto.preview_nested_values', {
          value1: structuredEntryCount(structuredValue)
        })}
      </Typography.Text>
    );
  }

  const visibleEntries = entries.slice(0, PREVIEW_NESTED_ENTRY_LIMIT);
  return (
    <Space orientation="vertical" size={2}>
      {visibleEntries.map(([key, entry]) => (
        <div key={key}>
          <PreviewFieldKey fieldKey={key} />
          <Typography.Text type="secondary">: </Typography.Text>
          <PreviewValue value={entry} depth={depth + 1} />
        </div>
      ))}
      {entries.length > visibleEntries.length ? (
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.preview_more_values', {
            value1: entries.length - visibleEntries.length
          })}
        </Typography.Text>
      ) : null}
    </Space>
  );
}

export function DataSourceResourcePreviewDrawer({
  dataSource,
  resource,
  preview,
  onClose
}: {
  dataSource: SettingsRuntimeExtensionDataSource;
  resource: SettingsDataSourceRemoteResource;
  preview: SettingsDataSourcePreview;
  onClose: () => void;
}) {
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;
  const rows = preview.output.rows;
  const fields = useMemo(
    () => Array.from(new Set(rows.flatMap((row) => Object.keys(row)))),
    [rows]
  );
  const visibleFields = fields.slice(0, PREVIEW_FIELD_LIMIT);
  const cellBoundedRowLimit =
    visibleFields.length === 0
      ? PREVIEW_ROW_LIMIT
      : Math.min(
          PREVIEW_ROW_LIMIT,
          Math.floor(PREVIEW_CELL_LIMIT / visibleFields.length)
        );
  const visibleRows = rows.slice(0, cellBoundedRowLimit);
  const omittedFieldCount = fields.length - visibleFields.length;
  const omittedRowCount = rows.length - visibleRows.length;
  const columns = useMemo<ColumnsType<Record<string, unknown>>>(
    () =>
      visibleFields.map((field) => ({
        title: <PreviewFieldKey fieldKey={field} />,
        key: field,
        render: (_, row) => <PreviewValue value={row[field]} />
      })),
    [visibleFields]
  );
  const rowKeys = useMemo(
    () => new Map(visibleRows.map((row, index) => [row, String(index)])),
    [visibleRows]
  );

  return (
    <Drawer
      title={i18nText('settings', 'auto.resource_preview_title', {
        value1: resource.display_name
      })}
      open
      placement={isMobile ? 'bottom' : 'right'}
      size={isMobile ? '85vh' : 760}
      onClose={onClose}
    >
      <Space orientation="vertical" size={16} style={{ width: '100%' }}>
        <Space orientation="vertical" size={2}>
          <Typography.Text>{dataSource.display_name}</Typography.Text>
          <Typography.Text type="secondary">
            <code className="data-model-panel__code-badge">
              {resource.resource_key}
            </code>
          </Typography.Text>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.preview_rows_loaded', {
              value1: rows.length
            })}
          </Typography.Text>
          {omittedFieldCount > 0 ? (
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.preview_fields_omitted', {
                value1: omittedFieldCount
              })}
            </Typography.Text>
          ) : null}
          {omittedRowCount > 0 ? (
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.preview_rows_omitted', {
                value1: omittedRowCount
              })}
            </Typography.Text>
          ) : null}
        </Space>

        {rows.length === 0 ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={i18nText('settings', 'auto.preview_no_rows')}
          />
        ) : isMobile ? (
          <ul
            aria-label={i18nText('settings', 'auto.preview_mobile_rows')}
            className="structured-list__items"
          >
            {visibleRows.map((row, index) => (
              <li
                className="structured-list__item"
                key={rowKeys.get(row) ?? index}
              >
                <Descriptions
                  size="small"
                  bordered
                  column={1}
                  title={i18nText('settings', 'auto.preview_row_title', {
                    value1: index + 1
                  })}
                  items={visibleFields.map((field) => ({
                    key: field,
                    label: <PreviewFieldKey fieldKey={field} />,
                    children: <PreviewValue value={row[field]} />
                  }))}
                />
              </li>
            ))}
          </ul>
        ) : (
          <Table
            rowKey={(row) => rowKeys.get(row) ?? 'preview-row'}
            size="small"
            columns={columns}
            dataSource={visibleRows}
            pagination={false}
            scroll={{ x: 'max-content' }}
          />
        )}
      </Space>
    </Drawer>
  );
}
