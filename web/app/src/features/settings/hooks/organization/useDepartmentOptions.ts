import { useState } from 'react';
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient
} from '@tanstack/react-query';
import {
  fetchSettingsDepartments,
  settingsDepartmentsQueryKey,
  type SettingsDepartment
} from '../../api/departments';
export function useDepartmentOptions(
  selected_ids: string[],
  initial: SettingsDepartment[] = [],
  enabled = true
) {
  const client = useQueryClient();
  const [prefix, setPrefix] = useState('');
  const ids = [...new Set(selected_ids)].sort().join(',');
  const selected = useQuery({
    queryKey: [...settingsDepartmentsQueryKey, 'options', 'selected', ids],
    queryFn: () =>
      fetchSettingsDepartments({
        ids,
        limit: Math.max(50, selected_ids.length)
      }),
    enabled: enabled && !!ids,
    retry: false
  });
  const query = useInfiniteQuery({
    queryKey: [...settingsDepartmentsQueryKey, 'options', prefix],
    queryFn: ({ pageParam }) =>
      fetchSettingsDepartments({
        ...(prefix ? { prefix } : {}),
        limit: 50,
        ...(pageParam ? { cursor: pageParam } : {})
      }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) =>
      page.has_more ? (page.next_cursor ?? undefined) : undefined,
    enabled,
    retry: false
  });
  const departments = [
    ...new Map(
      [
        ...(prefix
          ? initial.filter((item) => selected_ids.includes(item.id))
          : initial),
        ...(selected.data?.items ?? []),
        ...(query.data?.pages.flatMap((page) =>
          page.items.filter((item) => item.is_match)
        ) ?? [])
      ].map((item) => [item.id, item])
    ).values()
  ];
  return {
    departments,
    setPrefix,
    loading: query.isFetching || selected.isFetching,
    error: query.isError || selected.isError,
    hasMore: query.hasNextPage,
    loadMore: () => void query.fetchNextPage(),
    reload: () => {
      void client.resetQueries({
        queryKey: [...settingsDepartmentsQueryKey, 'options', prefix],
        exact: true
      });
      void selected.refetch();
    }
  };
}
