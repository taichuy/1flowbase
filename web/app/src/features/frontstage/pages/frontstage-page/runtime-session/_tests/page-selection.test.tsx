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
        autoSelectFirstPage: true,
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
    usePageTreeWorkspace({ initialPageTree, autoSelectFirstPage: true })
  );
  act(() => result.current.handleSelectPage('b'));
  expect(result.current.selectedPageId).toBe('b');
});

test('drag reordering retains the scoped route parent in the backend request', async () => {
  const onMovePageNode = vi.fn().mockResolvedValue(undefined);
  const { result } = renderHook(() =>
    usePageTreeWorkspace({
      initialPageTree,
      autoSelectFirstPage: true,
      pageTreeRootId: 'route-1',
      onMovePageNode
    })
  );
  await act(async () =>
    result.current.handleMoveNodeToPosition('b', 'a', 'before')
  );
  expect(result.current.pageTree.map((node) => node.id)).toEqual(['b', 'a']);
  expect(onMovePageNode).toHaveBeenCalledWith('b', {
    parentId: 'route-1',
    rank: '000000'
  });
});
