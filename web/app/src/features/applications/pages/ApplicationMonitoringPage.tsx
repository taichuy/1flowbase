import { useQuery } from '@tanstack/react-query';

import {
  applicationRuntimeActivityQueryKey,
  fetchApplicationRuntimeActivity
} from '../api/runtime';
import { RuntimeActivityPanel } from './ApplicationStatisticsPage';
import './application-monitoring-page.css';

export function ApplicationMonitoringPage({
  applicationId
}: {
  applicationId: string;
}) {
  const runtimeActivityQuery = useQuery({
    queryKey: applicationRuntimeActivityQueryKey(applicationId),
    queryFn: () => fetchApplicationRuntimeActivity(applicationId),
    refetchInterval: 5000
  });

  return (
    <div
      className="application-monitoring-page"
      data-testid="application-monitoring-page"
    >
      <RuntimeActivityPanel
        activity={runtimeActivityQuery.data}
        loading={runtimeActivityQuery.isPending}
        error={runtimeActivityQuery.isError}
      />
    </div>
  );
}
