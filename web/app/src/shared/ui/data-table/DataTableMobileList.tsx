import { Button, Checkbox, Empty, Listy, Spin } from 'antd';
import {
  useLayoutEffect,
  useRef,
  useState,
  type Key,
  type ReactNode
} from 'react';
import type { TableProps } from 'antd';
import type { DataTableColumn } from './data-table-state';
import { i18nText } from '../../i18n/text';
import './data-table-mobile-list.css';

export type DataTableMobilePagination = {
  resetKey?: string;
  showSelectAll?: boolean;
  hasMore: boolean;
  loading: boolean;
  failed: boolean;
  onLoadMore: () => Promise<unknown>;
};

export function DataTableMobileList<T extends object>({
  columns,
  items,
  rowKey,
  rowSelection,
  rowClassName,
  emptyText,
  pagination
}: {
  columns: Array<DataTableColumn<T>>;
  items: T[];
  rowKey: keyof T | ((record: T) => Key);
  rowSelection?: TableProps<T>['rowSelection'];
  rowClassName?: (record: T, index: number) => string;
  emptyText?: ReactNode;
  pagination: DataTableMobilePagination;
}) {
  const container = useRef<HTMLDivElement>(null);
  const footer = useRef<HTMLDivElement>(null);
  const inFlight = useRef(false);
  const [height, setHeight] = useState(400);
  useLayoutEffect(() => {
    if (!container.current) return;
    const element = container.current;
    const measure = () =>
      setHeight(
        Math.max(
          240,
          window.innerHeight -
            element.getBoundingClientRect().top -
            (footer.current?.offsetHeight ?? 0) -
            16
        )
      );
    const observer = new ResizeObserver(measure);
    measure();
    observer.observe(element);
    window.addEventListener('resize', measure);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', measure);
    };
  }, []);
  const getKey = (item: T) =>
    typeof rowKey === 'function' ? rowKey(item) : (item[rowKey] as Key);
  async function loadMore() {
    if (inFlight.current || pagination.loading || !pagination.hasMore) return;
    inFlight.current = true;
    try {
      await pagination.onLoadMore();
    } finally {
      inFlight.current = false;
    }
  }
  const renderValue = (column: DataTableColumn<T>, item: T, index: number) => {
    const value = column.dataIndex ? item[column.dataIndex] : undefined;
    return column.render
      ? column.render(value, item, index)
      : ((value as ReactNode) ?? '-');
  };
  const titleColumn = columns.find((column) => column.mobileRole === 'title');
  const fields = columns.filter((column) => !column.mobileRole);
  const actions = columns.filter((column) => column.mobileRole === 'actions');
  const selectedKeys = rowSelection?.selectedRowKeys ?? [];
  const selectableItems = items.filter(
    (item) => !rowSelection?.getCheckboxProps?.(item).disabled
  );
  const allSelected =
    selectableItems.length > 0 &&
    selectableItems.every((item) => selectedKeys.includes(getKey(item)));

  return (
    <div className="data-table-mobile">
      {rowSelection && pagination.showSelectAll !== false && (
        <div className="data-table-mobile__selection">
          <Checkbox
            checked={allSelected}
            indeterminate={
              !allSelected &&
              selectableItems.some((item) =>
                selectedKeys.includes(getKey(item))
              )
            }
            disabled={!selectableItems.length}
            onChange={(event) => {
              const selectableKeys = new Set(selectableItems.map(getKey));
              const keys = event.target.checked
                ? [...new Set([...selectedKeys, ...selectableKeys])]
                : selectedKeys.filter((key) => !selectableKeys.has(key));
              rowSelection.onChange?.(
                keys,
                items.filter((item) => keys.includes(getKey(item))),
                { type: 'all' }
              );
            }}
          >
            {i18nText('sharedUi', 'auto.list_select_loaded')}
          </Checkbox>
        </div>
      )}

      <div className="data-table-mobile__viewport" ref={container}>
        {items.length ? (
          <Listy<T>
            virtual
            height={height}
            items={items}
            rowKey={getKey}
            onScroll={(event) => {
              const { scrollTop, clientHeight, scrollHeight } =
                event.currentTarget;
              if (
                scrollHeight - scrollTop - clientHeight < 200 &&
                !pagination.failed
              )
                void loadMore();
            }}
            itemRender={(item, index) => (
              <article
                className={[
                  'data-table-mobile__record',
                  rowClassName?.(item, index)
                ]
                  .filter(Boolean)
                  .join(' ')}
                data-row-key={getKey(item)}
              >
                {(titleColumn || rowSelection) && (
                  <header className="data-table-mobile__header">
                    {rowSelection && (
                      <Checkbox
                        {...rowSelection.getCheckboxProps?.(item)}
                        checked={selectedKeys.includes(getKey(item))}
                        onChange={(event) => {
                          const key = getKey(item);
                          const keys = event.target.checked
                            ? [...selectedKeys, key]
                            : selectedKeys.filter((value) => value !== key);
                          rowSelection.onChange?.(
                            keys,
                            items.filter((record) =>
                              keys.includes(getKey(record))
                            ),
                            { type: 'single' }
                          );
                        }}
                      />
                    )}
                    {titleColumn && (
                      <div className="data-table-mobile__title">
                        {renderValue(titleColumn, item, index)}
                      </div>
                    )}
                  </header>
                )}
                <dl className="data-table-mobile__fields">
                  {fields.map((column) => (
                    <div className="data-table-mobile__field" key={column.key}>
                      <dt>{column.title}</dt>
                      <dd>{renderValue(column, item, index)}</dd>
                    </div>
                  ))}
                </dl>
                {actions.length > 0 && (
                  <footer className="data-table-mobile__actions">
                    {actions.map((column) => (
                      <span key={column.key}>
                        {renderValue(column, item, index)}
                      </span>
                    ))}
                  </footer>
                )}
              </article>
            )}
          />
        ) : (
          <Empty description={emptyText} />
        )}
      </div>
      <div ref={footer} className="data-table-mobile__pagination" role="status">
        {pagination.loading ? (
          <Spin size="small" />
        ) : pagination.failed ? (
          <Button onClick={() => void loadMore()}>
            {i18nText('sharedUi', 'auto.list_retry')}
          </Button>
        ) : pagination.hasMore ? (
          <Button type="text" onClick={() => void loadMore()}>
            {i18nText('sharedUi', 'auto.list_load_more')}
          </Button>
        ) : (
          <span>{i18nText('sharedUi', 'auto.list_end')}</span>
        )}
      </div>
    </div>
  );
}
