import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DataTable,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { DataTableLayout } from '../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';

export function EventsTable<T extends object>({
  columns,
  rows,
  rowKey,
  tableKey,
  loading = false
}: {
  columns: DataTableColumn<T>[];
  rows: T[];
  rowKey: keyof T | ((row: T) => string);
  tableKey: string;
  loading?: boolean;
}) {
  const { t } = useTranslation('settingsEvents');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: `settings.events.${tableKey}`
  });
  const currentPage = Math.min(
    page,
    Math.max(1, Math.ceil(rows.length / pageSize))
  );
  return (
    <DataTableLayout>
      <DataTable
        columns={columns}
        configuration={configuration}
        dataSource={rows.slice(
          (currentPage - 1) * pageSize,
          currentPage * pageSize
        )}
        rowKey={rowKey}
        loading={loading}
        emptyText={t('empty')}
        page={currentPage}
        pageSize={pageSize}
        total={rows.length}
        onPageChange={(next, size) => {
          setPage(next);
          setPageSize(size);
        }}
      />
    </DataTableLayout>
  );
}
