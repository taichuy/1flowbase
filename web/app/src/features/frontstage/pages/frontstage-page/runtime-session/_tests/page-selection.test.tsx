import { act, renderHook, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { usePageTreeWorkspace } from '../../use-page-tree-workspace';

const initialPageTree = [
  { id: 'a', title: 'A', kind: 'page' as const },
  { id: 'b', title: 'B', kind: 'page' as const }
];

test('route navigation does not temporarily replace the outgoing page runtime scope', async () => {
  const onNavigatePage = vi.fn();
  const { result, rerender } = renderHook(
    ({ pageId }) =>
      usePageTreeWorkspace({
        pageId,
        initialPageTree,
        onNavigatePage
      }),
    { initialProps: { pageId: 'a' } }
  );
  act(() => result.current.handleSelectPage('b'));
  expect(onNavigatePage).toHaveBeenCalledWith('b');
  expect(result.current.selectedPageId).toBe('a');
  rerender({ pageId: 'b' });
  await waitFor(() => expect(result.current.selectedPageId).toBe('b'));
});

test('standalone page selection without a route owner remains interactive', () => {
  const { result } = renderHook(() =>
    usePageTreeWorkspace({ initialPageTree })
  );
  act(() => result.current.handleSelectPage('b'));
  expect(result.current.selectedPageId).toBe('b');
});
