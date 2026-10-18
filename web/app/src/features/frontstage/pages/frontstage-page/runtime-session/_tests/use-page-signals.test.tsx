import { renderHook } from '@testing-library/react';
import { expect, test } from 'vitest';
import type { FrontstageBlockInstance } from '../../../../lib/page-document';
import { usePageSignals } from '../use-page-signals';

const producer = {
  id: 'producer', ports: { inputs: [], outputs: [{ name: 'total', schema: { type: 'integer' } }] }
} as unknown as FrontstageBlockInstance;
const consumer = {
  id: 'consumer', ports: {
    inputs: [{ name: 'total', schema: { type: 'integer' }, source: {
      block_id: 'producer', output: 'total', scope: 'page', tab_id: 'tab-1'
    } }], outputs: []
  }
} as unknown as FrontstageBlockInstance;

test('keeps live epochs on block updates, shares page signals across tabs and resets a changed page', () => {
  const initial = { pageId: 'page-1', tabId: 'tab-1', rootUid: 'root-1', blocks: [producer, consumer] };
  const { result, rerender, unmount } = renderHook(usePageSignals, { initialProps: initial });
  const first = result.current!;
  first.beginInstance('producer', 'epoch-1');
  expect(first.commit('producer', 'epoch-1', { total: 3 }).ok).toBe(true);
  rerender({ ...initial, blocks: [...initial.blocks] });
  expect(result.current).toBe(first);
  expect(first.commit('producer', 'epoch-1', { total: 4 }).ok).toBe(true);
  rerender({ ...initial, tabId: 'tab-2' });
  const second = result.current!;
  expect(second).not.toBe(first);
  expect(second.inputsFor('consumer')).toEqual({ total: 4 });
  expect(first.commit('producer', 'epoch-1', { total: 5 }).stale).toBe(true);
  rerender({ ...initial, tabId: 'tab-2', rootUid: 'root-2' });
  expect(result.current).not.toBe(second);
  expect(result.current!.inputsFor('consumer')).toEqual({ total: 4 });
  rerender({ ...initial, pageId: 'page-2' });
  expect(result.current!.inputsFor('consumer')).toEqual({});
  const last = result.current!;
  last.beginInstance('producer', 'epoch-last');
  unmount();
  expect(last.commit('producer', 'epoch-last', { total: 9 }).stale).toBe(true);
});
