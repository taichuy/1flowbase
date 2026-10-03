import { infiniteQueryOptions } from '@tanstack/react-query';
import type { WorkflowTrajectoryOptions } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';

export function workflowTrajectoryQueryOptions(
  runId: string,
  loader: ConversationLogTraceLoader,
  filters: WorkflowTrajectoryOptions
) {
  return infiniteQueryOptions({
    queryKey: ['workflow-trajectory', runId, filters],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadWorkflowTrajectory!(runId, pageParam, filters),
    getNextPageParam: (page, all, cursor) =>
      page.next_cursor &&
      page.next_cursor !== cursor &&
      !all
        .slice(0, -1)
        .some((previous) => previous.next_cursor === page.next_cursor)
        ? page.next_cursor
        : undefined,
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
}
