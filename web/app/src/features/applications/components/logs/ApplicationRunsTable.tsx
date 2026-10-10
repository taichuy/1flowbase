import type { DataTableMobilePagination } from '../../../../shared/ui/data-table/DataTableMobileList';
import { Button } from 'antd';
import { useMemo } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn,
  type DataTableRowSelection
} from '../../../../shared/ui/data-table/DataTable';
import type { ApplicationRunSummary } from '../../api/runtime';
import type { ApplicationRunsTableConfiguration } from './useApplicationRunsTableConfiguration';

export function ApplicationRunsTableColumnSettings({
  columns,
  configuration
}: {
  columns: Array<DataTableColumn<ApplicationRunSummary>>;
  configuration: ApplicationRunsTableConfiguration;
}) {
  return (
    <DataTableColumnSettings<ApplicationRunSummary>
      className="application-runs-table__column-selector"
      columns={columns}
      configuration={configuration}
    />
  );
}

export function ApplicationRunsTable({
  loading = false,
  mobileList,
  page,
  pageSize,
  total,
  runs,
  columns,
  rowSelection,
  selectedRunId,
  configuration,
  onPageChange,
  onSelectRun
}: {
  loading?: boolean;
  mobileList?: DataTableMobilePagination;
  page: number;
  pageSize: number;
  total: number;
  runs: ApplicationRunSummary[];
  columns: Array<DataTableColumn<ApplicationRunSummary>>;
  rowSelection?: DataTableRowSelection<ApplicationRunSummary>;
  selectedRunId?: string | null;
  configuration: ApplicationRunsTableConfiguration;
  onPageChange: (page: number) => void;
  onSelectRun: (run: ApplicationRunSummary) => void;
}) {
  const { t } = useTranslation('applications');
  const tableColumns = useMemo<Array<DataTableColumn<ApplicationRunSummary>>>(
    () =>
      columns.map((column) => {
        if (column.key !== 'action') {
          return column;
        }

        return {
          ...column,
          render: (_value: unknown, run: ApplicationRunSummary): ReactNode => (
            <Button type="link" onClick={() => onSelectRun(run)}>
              {t('auto.view_run_details')}
            </Button>
          )
        };
      }),
    [columns, onSelectRun, t]
  );

  return (
    <DataTable<ApplicationRunSummary>
      className="application-runs-table"
      columns={tableColumns}
      configuration={configuration}
      dataSource={runs}
      loading={loading}
      mobileList={mobileList}
      page={page}
      pageSize={pageSize}
      rowClassName={(record) =>
        record.id === selectedRunId ? 'application-runs-table__row--active' : ''
      }
      rowKey={(record) => record.id}
      rowSelection={rowSelection}
      total={total}
      onPageChange={onPageChange}
    />
  );
}
