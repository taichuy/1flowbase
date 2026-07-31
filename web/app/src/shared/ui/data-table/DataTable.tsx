import { CheckOutlined } from '@ant-design/icons';
import { Button, Pagination, Select, Table } from 'antd';
import { useCallback, useMemo } from 'react';
import type { Key, MouseEvent, ReactNode, ThHTMLAttributes } from 'react';
import type { TableProps } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import {
  DEFAULT_MIN_COLUMN_WIDTH,
  getColumnMinWidth,
  getDefaultColumnWidths,
  getDefaultVisibleKeys,
  normalizeVisibleKeys,
  type DataTableColumn,
  type DataTableConfiguration
} from './data-table-state';
import './data-table.css';
import { i18nText } from '../../i18n/text';

export type { DataTableColumn, DataTableConfiguration };
export type DataTableRowSelection<T extends object> =
  TableProps<T>['rowSelection'];

type ResizeHeaderCellProps = ThHTMLAttributes<HTMLElement> & {
  onResizeMouseDown?: (event: MouseEvent<HTMLElement>) => void;
};

function ResizeHeaderCell({
  className,
  children,
  onResizeMouseDown,
  ...rest
}: ResizeHeaderCellProps) {
  return (
    <th {...rest} className={`data-table__header-cell ${className ?? ''}`}>
      <span className="data-table__header-title">{children}</span>
      <span
        aria-hidden="true"
        className="data-table__header-resize-handle"
        onMouseDown={onResizeMouseDown}
      />
    </th>
  );
}

export function DataTableColumnSettings<T extends object>({
  ariaLabel = i18nText('sharedUi', 'auto.field_configuration'),
  className,
  columns,
  configuration,
  placeholder = i18nText('sharedUi', 'auto.field_configuration'),
  resetLabel = i18nText('sharedUi', 'auto.reset_default_fields')
}: {
  ariaLabel?: string;
  className?: string;
  columns: Array<DataTableColumn<T>>;
  configuration: DataTableConfiguration;
  placeholder?: string;
  resetLabel?: string;
}) {
  const { visibleColumnKeys, setVisibleColumnKeys, setColumnWidths } =
    configuration;

  function handleColumnsChange(nextVisible: string[]) {
    if (nextVisible.length === 0) {
      return;
    }

    const nextVisibleKeys = new Set(nextVisible);
    const next: string[] = [];
    for (const column of columns) {
      if (nextVisibleKeys.has(column.key)) {
        next.push(column.key);
      }
    }

    setVisibleColumnKeys(next);
  }

  const columnSelectOptions = columns.map((column) => ({
    label: column.title,
    value: column.key,
    disabled:
      visibleColumnKeys.length === 1 && visibleColumnKeys.includes(column.key)
  }));

  return (
    <Select<string[]>
      aria-label={ariaLabel}
      className={['data-table__column-selector', className]
        .filter(Boolean)
        .join(' ')}
      classNames={{
        popup: {
          root: 'data-table__column-selector-popup'
        }
      }}
      listHeight={260}
      maxTagCount="responsive"
      mode="multiple"
      optionFilterProp="label"
      options={columnSelectOptions}
      placement="bottomRight"
      placeholder={placeholder}
      popupMatchSelectWidth
      value={visibleColumnKeys}
      virtual={false}
      popupRender={(originNode) => (
        <div className="data-table__column-selector-popup-inner">
          {originNode}
          <div className="data-table__column-selector-footer">
            <Button
              block
              icon={<CheckOutlined aria-hidden="true" />}
              size="small"
              type="text"
              onClick={() => {
                setVisibleColumnKeys(getDefaultVisibleKeys(columns));
                setColumnWidths(getDefaultColumnWidths(columns));
              }}
            >
              {resetLabel}
            </Button>
          </div>
        </div>
      )}
      onChange={handleColumnsChange}
    />
  );
}

export function DataTable<T extends object>({
  className,
  columns,
  configuration,
  dataSource,
  loading = false,
  page,
  pageSize,
  rowClassName,
  rowKey,
  toolbar,
  onRow,
  rowSelection,
  total,
  onPageChange
}: {
  className?: string;
  columns: Array<DataTableColumn<T>>;
  configuration: DataTableConfiguration;
  dataSource: T[];
  loading?: boolean;
  page: number;
  pageSize: number;
  rowClassName?: (record: T, index: number) => string;
  rowKey: keyof T | ((record: T) => Key);
  toolbar?: ReactNode;
  onRow?: TableProps<T>['onRow'];
  rowSelection?: DataTableRowSelection<T>;
  total: number;
  onPageChange: (page: number) => void;
}) {
  const { visibleColumnKeys, columnWidths, setColumnWidths } = configuration;

  const startResize = useCallback(
    (
      columnKey: string,
      minWidth: number,
      startWidth: number,
      event: MouseEvent<HTMLElement>
    ) => {
      event.preventDefault();
      event.stopPropagation();

      const initialX = event.clientX;
      const start = Math.max(minWidth, startWidth);

      function onMouseMove(mouseEvent: globalThis.MouseEvent) {
        const nextWidth = Math.max(
          minWidth,
          Math.round(start + (mouseEvent.clientX - initialX))
        );

        setColumnWidths((current) => {
          if (current[columnKey] === nextWidth) {
            return current;
          }

          return {
            ...current,
            [columnKey]: nextWidth
          };
        });
      }

      function onMouseUp() {
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
        document.body.style.removeProperty('cursor');
        document.body.style.removeProperty('user-select');
      }

      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    },
    [setColumnWidths]
  );

  const appliedVisibleColumnKeys = useMemo(() => {
    return normalizeVisibleKeys(columns, visibleColumnKeys);
  }, [columns, visibleColumnKeys]);

  const tableColumns = useMemo<ColumnsType<T>>(() => {
    const visibleFields = columns.filter((column) =>
      appliedVisibleColumnKeys.includes(column.key)
    );

    return visibleFields.map((column) => {
      const minWidth = getColumnMinWidth(column);
      const width =
        columnWidths[column.key] && columnWidths[column.key] >= minWidth
          ? columnWidths[column.key]
          : column.width;

      return {
        key: column.key,
        title: column.title,
        dataIndex: column.dataIndex as string | undefined,
        width,
        ellipsis: column.ellipsis,
        render: (value: unknown, record: T, index: number) => {
          if (column.render) {
            return column.render(value, record, index);
          }

          return (value as string | null | number) ?? '-';
        },
        onHeaderCell: () =>
          ({
            onResizeMouseDown: (event: MouseEvent<HTMLElement>) =>
              startResize(column.key, minWidth, width, event),
            width
          }) as ResizeHeaderCellProps
      };
    });
  }, [appliedVisibleColumnKeys, columnWidths, columns, startResize]);

  const fixedTableWidth = useMemo(() => {
    return tableColumns.reduce((sum, column) => {
      const fixedWidth =
        typeof column.width === 'number'
          ? column.width
          : DEFAULT_MIN_COLUMN_WIDTH;

      return sum + fixedWidth;
    }, 0);
  }, [tableColumns]);

  return (
    <section className={['data-table', className].filter(Boolean).join(' ')}>
      <div className="data-table__scroll-area">
        <Table<T>
          rowKey={rowKey as string | ((record: T) => Key)}
          dataSource={dataSource}
          loading={loading}
          style={{ minWidth: fixedTableWidth }}
          tableLayout="fixed"
          components={{
            header: {
              cell: ResizeHeaderCell
            }
          }}
          pagination={false}
          title={
            toolbar
              ? () => <div className="data-table__toolbar">{toolbar}</div>
              : undefined
          }
          onRow={onRow}
          rowClassName={rowClassName}
          rowSelection={rowSelection}
          columns={tableColumns}
        />
      </div>
      <Pagination
        className="data-table__pagination"
        current={page}
        pageSize={pageSize}
        total={total}
        showSizeChanger={false}
        showTotal={(paginationTotal) =>
          i18nText('sharedUi', 'auto.total_items', { value1: paginationTotal })
        }
        onChange={onPageChange}
      />
    </section>
  );
}
