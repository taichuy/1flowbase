import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { type PropsWithChildren } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import { fetchApplicationRuns } from '../../../../api/runtime';
import { useApplicationRunsQuery } from '../use-application-runs-query';

vi.mock('../../../../api/runtime', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../api/runtime')>()),
  fetchApplicationRuns: vi.fn()
}));
const fetchRuns = vi.mocked(fetchApplicationRuns);
const row = (id: string) =>
  ({ id }) as Awaited<ReturnType<typeof fetchApplicationRuns>>['items'][number];
const page = (ids: string[], total = 3) =>
  ({ items: ids.map(row), total }) as Awaited<
    ReturnType<typeof fetchApplicationRuns>
  >;
beforeEach(() => vi.resetAllMocks());
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } }
  });
  const wrapper = ({ children }: PropsWithChildren) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    ({ search, mobile }) =>
      useApplicationRunsQuery(
        'app',
        { page: 1, pageSize: 2, titleIncludes: search },
        mobile
      ),
    { wrapper, initialProps: { search: '', mobile: true } }
  );
}

test('appends pages, deduplicates records and resets on filters and refresh', async () => {
  fetchRuns
    .mockResolvedValueOnce(page(['a', 'b']))
    .mockResolvedValueOnce(page(['b', 'c']));
  const { result, rerender } = setup();
  await waitFor(() =>
    expect(result.current.data?.items.map((item) => item.id)).toEqual([
      'a',
      'b'
    ])
  );
  await act(async () => {
    await result.current.mobileList?.onLoadMore();
  });
  await waitFor(() =>
    expect(result.current.data?.items.map((item) => item.id)).toEqual([
      'a',
      'b',
      'c'
    ])
  );
  expect(result.current.mobileList?.hasMore).toBe(false);
  const previousResetKey = result.current.mobileList?.resetKey;
  fetchRuns.mockResolvedValueOnce(page(['filtered'], 1));
  rerender({ search: 'filtered', mobile: true });
  await waitFor(() =>
    expect(result.current.data?.items.map((item) => item.id)).toEqual([
      'filtered'
    ])
  );
  expect(result.current.mobileList?.resetKey).not.toBe(previousResetKey);
  fetchRuns.mockResolvedValueOnce(page(['fresh'], 1));
  await act(async () => {
    await result.current.refresh();
  });
  await waitFor(() =>
    expect(result.current.data?.items.map((item) => item.id)).toEqual(['fresh'])
  );
  expect(fetchRuns).toHaveBeenLastCalledWith(
    'app',
    expect.objectContaining({ page: 1, cacheMode: 'refresh' })
  );
});

test('keeps loaded rows after next-page failure and allows retry', async () => {
  fetchRuns
    .mockResolvedValueOnce(page(['a', 'b']))
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValueOnce(page(['c']));
  const { result } = setup();
  await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
  await act(async () => {
    await result.current.mobileList?.onLoadMore();
  });
  await waitFor(() => expect(result.current.mobileList?.failed).toBe(true));
  expect(result.current.isError).toBe(false);
  expect(result.current.data?.items).toHaveLength(2);
  await act(async () => {
    await result.current.mobileList?.onLoadMore();
  });
  await waitFor(() => expect(result.current.data?.items).toHaveLength(3));
});

test('uses ordinary page data when switching to desktop', async () => {
  fetchRuns.mockResolvedValue(page(['a', 'b']));
  const { result, rerender } = setup();
  await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
  rerender({ search: '', mobile: false });
  await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
  expect(result.current.mobileList).toBeUndefined();
});
