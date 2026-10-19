import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  hashKey,
  useQuery,
  useQueryClient,
  type QueryKey
} from '@tanstack/react-query';
import type { TrajectoryOverview } from '@1flowbase/api-client';

export function useTrajectoryWindow<
  Row,
  Index,
  Page extends {
    items: Row[];
    overview?: TrajectoryOverview<Index>;
  }
>({
  queryKey,
  enabled,
  loadPage,
  getRowKey,
  getIndexKey
}: {
  queryKey: QueryKey;
  enabled: boolean;
  loadPage: (focusId: string | undefined, indexOnly: boolean) => Promise<Page>;
  getRowKey: (row: Row) => string;
  getIndexKey: (row: Index) => string;
}) {
  const client = useQueryClient();
  const scope = hashKey(queryKey);
  const currentScope = useRef(scope);
  currentScope.current = scope;
  const load = useRef(loadPage);
  load.current = loadPage;
  const [visible, setVisible] = useState(
    () => document.visibilityState !== 'hidden'
  );
  useEffect(() => {
    const update = () => setVisible(document.visibilityState !== 'hidden');
    document.addEventListener('visibilitychange', update);
    return () => document.removeEventListener('visibilitychange', update);
  }, []);
  const overview = useQuery({
    queryKey: [...queryKey, 'index'],
    queryFn: () => load.current(undefined, true),
    enabled: enabled && visible,
    staleTime: 60_000,
    refetchOnWindowFocus: false,
    retry: false
  });
  const first = useQuery({
    queryKey: [...queryKey, 'initial-window'],
    queryFn: () => load.current(undefined, false),
    enabled: enabled && visible,
    staleTime: 60_000,
    refetchOnWindowFocus: false,
    retry: false
  });
  type Windows = {
    scope: string;
    pages: Page[];
    fetching: boolean;
    error: unknown;
  };
  const [windows, setWindows] = useState<Windows>({
    scope,
    pages: [],
    fetching: false,
    error: null
  });
  const [viewport, setViewport] = useState<{ scope: string; keys: string[] }>({
    scope,
    keys: []
  });
  // Explicit navigation is retained independently of incidental viewport reports.
  const [focus, setFocus] = useState<{ scope: string; key: string } | null>(null);
  const requestFocus = useCallback(
    (key: string) => setFocus({ scope, key }),
    [scope]
  );
  const inFlight = useRef<string | null>(null);
  const windowPages = useMemo(
    () => (windows.scope === scope ? windows.pages : []),
    [windows, scope]
  );
  const error = windows.scope === scope ? windows.error : null;
  const allPages = useMemo(
    () => (first.data ? [first.data, ...windowPages] : windowPages),
    [first.data, windowPages]
  );
  const loaded = useMemo(() => {
    const map = new Map<string, Row>();
    for (const page of allPages)
      for (const row of page.items) map.set(getRowKey(row), row);
    return map;
  }, [allPages, getRowKey]);
  const validKeys = useMemo(
    () =>
      new Set([
        ...(overview.data?.overview?.items.map(getIndexKey) ?? []),
        ...(overview.data?.overview?.request_roots?.map((row) => row.id) ?? [])
      ]),
    [overview.data, getIndexKey]
  );
  const onVisibleKeys = useCallback(
    (keys: string[]) => {
      setViewport((previous) =>
        previous.scope === scope &&
        previous.keys.length === keys.length &&
        previous.keys.every((key, i) => key === keys[i])
          ? previous
          : { scope, keys }
      );
    },
    [scope]
  );
  const focusMissing =
    focus?.scope === scope && validKeys.has(focus.key) && !loaded.has(focus.key)
      ? focus.key
      : undefined;
  const missing =
    focusMissing ??
    (viewport.scope === scope
      ? viewport.keys.find((key) => validKeys.has(key) && !loaded.has(key))
      : undefined);
  useEffect(() => {
    if (
      !enabled ||
      !visible ||
      first.isPending ||
      first.isError ||
      overview.isError ||
      error ||
      !missing ||
      inFlight.current === scope
    )
      return;
    const timer = window.setTimeout(() => {
      if (
        currentScope.current !== scope ||
        document.visibilityState === 'hidden'
      )
        return;
      inFlight.current = scope;
      setWindows((previous) => ({
        scope,
        pages: previous.scope === scope ? previous.pages : [],
        fetching: true,
        error: null
      }));
      void client
        .fetchQuery({
          queryKey: [...queryKey, 'window', missing],
          queryFn: () => load.current(missing, false),
          staleTime: 60_000,
          retry: false
        })
        .then((page) => {
          if (!page.items.some((row) => getRowKey(row) === missing))
            throw new Error('Trajectory focus is absent from its window');
          if (currentScope.current !== scope) return;
          setWindows((previous) => ({
            scope,
            pages: [...(previous.scope === scope ? previous.pages : []), page],
            fetching: false,
            error: null
          }));
        })
        .catch((cause: unknown) => {
          if (currentScope.current === scope)
            setWindows((previous) => ({
              scope,
              pages: previous.scope === scope ? previous.pages : [],
              fetching: false,
              error: cause
            }));
        })
        .finally(() => {
          if (inFlight.current === scope) inFlight.current = null;
        });
    }, 100);
    return () => window.clearTimeout(timer);
  }, [
    client,
    enabled,
    visible,
    first.isPending,
    first.isError,
    overview.isError,
    error,
    missing,
    scope,
    queryKey,
    getRowKey,
    windowPages
  ]);
  const { isError: overviewIsError, refetch: refetchOverview } = overview;
  const { isError: firstIsError, refetch: refetchFirst } = first;
  const retry = useCallback(() => {
    setWindows((previous) => ({
      scope,
      pages: previous.scope === scope ? previous.pages : [],
      fetching: false,
      error: null
    }));
    if (overviewIsError) void refetchOverview();
    if (firstIsError) void refetchFirst();
  }, [scope, firstIsError, refetchFirst, overviewIsError, refetchOverview]);
  const refetch = useCallback(async () => {
    await Promise.all([refetchOverview(), refetchFirst()]);
    if (currentScope.current === scope)
      setWindows({ scope, pages: [], fetching: false, error: null });
  }, [refetchOverview, refetchFirst, scope]);
  return {
    overview,
    pages: {
      data: first.data ? { pages: allPages } : undefined,
      isPending: first.isPending || overview.isPending,
      isLoading: first.isLoading || overview.isLoading,
      isFetching:
        first.isFetching ||
        overview.isFetching ||
        (windows.scope === scope && windows.fetching),
      isError: first.isError || overview.isError || Boolean(error),
      error: first.error ?? overview.error ?? error,
      refetch
    },
    loaded,
    items: useMemo(() => [...loaded.values()], [loaded]),
    initialKey: first.data?.items[0]
      ? getRowKey(first.data.items[0])
      : undefined,
    onVisibleKeys,
    requestFocus,
    retry
  };
}
