import { useEffect } from 'react';

/** Load summary pages serially, yielding between pages; hidden views retain their cache. */
export function useProgressiveTrajectory(
  active: boolean,
  pages: {
    hasNextPage: boolean;
    isFetching: boolean;
    isError: boolean;
    fetchNextPage: () => Promise<unknown>;
  }
) {
  const { hasNextPage, isFetching, isError, fetchNextPage } = pages;
  useEffect(() => {
    if (!active || !hasNextPage || isFetching || isError) return;
    const timer = window.setTimeout(() => void fetchNextPage(), 100);
    return () => window.clearTimeout(timer);
  }, [active, hasNextPage, isFetching, isError, fetchNextPage]);
}
