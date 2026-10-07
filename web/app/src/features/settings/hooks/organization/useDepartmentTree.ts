import { useMemo, useState } from 'react';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import {
  fetchSettingsDepartments,
  settingsDepartmentsQueryKey,
  type DepartmentTreeItem,
  type DepartmentListParams
} from '../../api/departments';
export function useDepartmentTree(enabled = true) {
  const client = useQueryClient();
  const [prefix, setPrefixValue] = useState('');
  const [requests, setRequests] = useState<DepartmentListParams[]>([{}]);
  const queries = useQueries({
    queries: requests.map((params) => ({
      queryKey: [...settingsDepartmentsQueryKey, 'tree', params],
      queryFn: () => fetchSettingsDepartments({ ...params, limit: 50 }),
      enabled,
      retry: false
    }))
  });
  const items = useMemo(() => {
    const merged = new Map<string, DepartmentTreeItem>();
    for (const item of queries.flatMap((query) => query.data?.items ?? [])) {
      merged.set(item.id, {
        ...item,
        is_match: item.is_match || merged.get(item.id)?.is_match === true
      });
    }
    return [...merged.values()];
  }, [queries]);
  const loadChildren = async (parent_id: string) => {
    if (
      !prefix.trim() &&
      !requests.some((request) => request.parent_id === parent_id)
    )
      setRequests((current) => [...current, { parent_id }]);
  };
  const groups = requests.flatMap((params, index) => {
    const page = queries[index].data;
    if (!page?.has_more || !page.next_cursor) return [];
    if (
      requests.some(
        (request) =>
          request.parent_id === params.parent_id &&
          request.prefix === params.prefix &&
          request.cursor === page.next_cursor
      )
    )
      return [];
    return [
      {
        parent_id: params.parent_id,
        loadMore: () =>
          setRequests((current) => [
            ...current,
            { ...params, cursor: page.next_cursor! }
          ])
      }
    ];
  });
  const reload = () => {
    const normalized = prefix.trim();
    const firstPage = normalized ? { prefix: normalized } : {};
    setRequests([firstPage]);
    void client.resetQueries({
      queryKey: [...settingsDepartmentsQueryKey, 'tree', firstPage],
      exact: true
    });
  };
  const setPrefix = (value: string) => {
    const next = value.trim();
    setPrefixValue(value);
    if (next !== prefix.trim()) setRequests([next ? { prefix: next } : {}]);
  };
  return {
    items,
    prefix,
    setPrefix,
    loadChildren,
    groups,
    reload,
    loading: enabled && queries.some((query) => query.isLoading),
    error: queries.some((query) => query.isError),
    isSuccess: queries[0]?.isSuccess ?? false
  };
}
