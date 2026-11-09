import { useState, type ReactNode } from 'react';
import { Flex, Grid, type TableProps } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { DataTableLayout } from '../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import { navigateEventState, useEventParameter } from './event-navigation';

export function EventsTable<T extends object>({
  columns,
  rows,
  rowKey,
  tableKey,
  loading = false,
  toolbarStart,
  toolbar,
  urlState = false,
  emptyText,
  onRow,
  rowClassName,
  mobileRecords
}: {
  columns: DataTableColumn<T>[];
  rows: T[];
  rowKey: keyof T | ((row: T) => string);
  tableKey: string;
  loading?: boolean;
  toolbarStart?: ReactNode;
  toolbar?: ReactNode;
  urlState?: boolean;
  emptyText?: ReactNode;
  onRow?: TableProps<T>['onRow'];
  rowClassName?: (record: T, index: number) => string;
  mobileRecords?: string;
}) {
  const { t } = useTranslation('settingsEvents');
  const screens = Grid.useBreakpoint();
  const showRecords =
    mobileRecords !== undefined &&
    screens.md === false &&
    (screens.xs === true || screens.sm === true);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const requestedPage = Number(useEventParameter(`${tableKey}_page`));
  const requestedSize = Number(useEventParameter(`${tableKey}_size`));
  const effectiveSize =
    urlState && [10, 20, 50, 100].includes(requestedSize)
      ? requestedSize
      : pageSize;
  const effectivePage =
    urlState && Number.isSafeInteger(requestedPage) && requestedPage > 0
      ? requestedPage
      : page;
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: `settings.events.${tableKey}`
  });
  const currentPage = Math.min(
    effectivePage,
    Math.max(1, Math.ceil(rows.length / effectiveSize))
  );
  const changePage = (next: number, size: number) => {
    if (urlState)
      navigateEventState({
        [`${tableKey}_page`]: String(next),
        [`${tableKey}_size`]: String(size)
      });
    else {
      setPage(next);
      setPageSize(size);
    }
  };
  return (
    <DataTableLayout>
      <DataTable
        columns={columns}
        configuration={configuration}
        dataSource={rows.slice(
          showRecords ? 0 : (currentPage - 1) * effectiveSize,
          currentPage * effectiveSize
        )}
        rowKey={rowKey}
        loading={loading}
        onRow={onRow}
        rowClassName={rowClassName}
        mobileList={
          showRecords
            ? {
                resetKey: mobileRecords,
                hasMore: currentPage * effectiveSize < rows.length,
                loading,
                failed: false,
                onLoadMore: async () =>
                  changePage(currentPage + 1, effectiveSize)
              }
            : undefined
        }
        toolbar={
          <Flex justify="space-between" align="center" gap={12} wrap>
            {toolbarStart}
            <Flex
              align="center"
              gap={8}
              wrap
              style={{ marginInlineStart: 'auto' }}
            >
              {toolbar}
              <DataTableColumnSettings
                columns={columns}
                configuration={configuration}
              />
            </Flex>
          </Flex>
        }
        emptyText={emptyText ?? t('empty')}
        page={currentPage}
        pageSize={effectiveSize}
        pageSizeOptions={[10, 20, 50, 100]}
        total={rows.length}
        onPageChange={changePage}
      />
    </DataTableLayout>
  );
}
