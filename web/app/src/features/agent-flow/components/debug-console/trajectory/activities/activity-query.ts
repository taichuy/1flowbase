import { useInfiniteQuery } from '@tanstack/react-query';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceProjectionStatus
} from '../../conversation-log-trace-model';
import { useProgressiveTrajectory } from '../use-progressive-trajectory';

export type ActivityPages = {
  data?: {
    pages: { projection_status?: ConversationLogTraceProjectionStatus }[];
  };
};
export function unfinishedProjection(pages: ActivityPages) {
  return pages.data?.pages
    .map((page) => page.projection_status)
    .find((status) => status && status.projection_status !== 'succeeded');
}

export type Scope = {
  runId: string;
  loader: ConversationLogTraceLoader;
  active: boolean;
};

export function useChildren(scope: Scope, parent: string, enabled: boolean) {
  const pages = useInfiniteQuery({
    queryKey: ['trajectory-activity-children', scope.runId, parent],
    enabled: scope.active && enabled,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      scope.loader.loadChildren(scope.runId, parent, pageParam),
    getNextPageParam: (page, all, cursor) => {
      const next = page.page_info.next_cursor;
      return page.page_info.has_more &&
        next &&
        next !== cursor &&
        !all.slice(0, -1).some((p) => p.page_info.next_cursor === next)
        ? next
        : undefined;
    },
    refetchInterval: (query) =>
      scope.active &&
      ['pending', 'running'].includes(
        query.state.data?.pages.at(-1)?.projection_status?.projection_status ??
          ''
      )
        ? 1000
        : false,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(scope.active && enabled, pages);
  return pages;
}
