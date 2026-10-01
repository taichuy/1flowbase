import { type DataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import type { DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import type { ApplicationRunSummary } from '../../api/runtime';

const USER_PREFERENCE_KEY = 'applications.logs.runs';

export type ApplicationRunsTableConfiguration = DataTableConfiguration;

export function useApplicationRunsTableConfiguration(
  columns: Array<DataTableColumn<ApplicationRunSummary>>,
  preferenceKey = USER_PREFERENCE_KEY
): ApplicationRunsTableConfiguration {
  return useUserPreferenceDataTableConfiguration<ApplicationRunSummary>({
    columns,
    preferenceKey
  });
}
