import { describe, expect, test } from 'vitest';

import {
  createChildContainerUrl,
  getRootPageBlockIds,
  resolveChildContainerCloseTarget,
  resolveChildContainerEvent,
  resolveChildContainerRuntime
} from '../lib/child-container-runtime';
import type { ChildContainerNode } from '../lib/child-container-tree';

const containers: ChildContainerNode[] = [
  {
    id: 'root-drawer',
    ownerBlockId: 'launcher',
    parentId: null,
    rank: '001000',
    presentation: 'drawer',
    title: 'Root',
    blockIds: ['drawer-content']
  },
  {
    id: 'child-modal',
    ownerBlockId: 'drawer-content',
    parentId: 'root-drawer',
    rank: '001000',
    presentation: 'modal',
    title: 'Child',
    blockIds: ['modal-content']
  }
];

describe('frontstage child container runtime contract', () => {
  test('AC-005 projects only container_id while preserving URL state and restores root-to-current', () => {
    expect(
      createChildContainerUrl(
        '/materials/page-1?design=true&view=table#section',
        'child-modal'
      )
    ).toBe(
      '/materials/page-1?design=true&view=table&container_id=child-modal#section'
    );

    const resolution = resolveChildContainerRuntime(containers, 'child-modal');
    expect(resolution.path.map(({ id }) => id)).toEqual([
      'root-drawer',
      'child-modal'
    ]);
    expect(resolution.current?.id).toBe('child-modal');
    expect(resolution.diagnostics).toEqual([]);
  });

  test('AC-005/006 opens from the explicit native event, closes to parent/root, and rejects unknown ids', () => {
    expect(
      resolveChildContainerEvent(containers, {
        sourceBlockId: 'launcher',
        name: 'open_child_container',
        payload: { container_id: 'root-drawer', context: { material: 1 } }
      })
    ).toEqual({ containerId: 'root-drawer', diagnostic: null });
    expect(resolveChildContainerCloseTarget(containers, 'child-modal')).toBe(
      'root-drawer'
    );
    expect(resolveChildContainerCloseTarget(containers, 'root-drawer')).toBe(
      null
    );
    expect(
      resolveChildContainerRuntime(containers, 'missing').diagnostics
    ).toEqual([expect.objectContaining({ code: 'unknown_child_container' })]);
    expect(
      resolveChildContainerEvent(containers, {
        sourceBlockId: 'launcher',
        name: 'open_child_container',
        payload: { container_id: 'missing' }
      }).diagnostic
    ).toEqual(expect.objectContaining({ code: 'unknown_child_container' }));
  });

  test('AC-003/007 filters assigned blocks from the root while historical pages remain unchanged', () => {
    expect(
      getRootPageBlockIds(
        ['launcher', 'drawer-content', 'modal-content', 'footer'],
        containers
      )
    ).toEqual(['launcher', 'footer']);
    expect(getRootPageBlockIds(['launcher', 'footer'], [])).toEqual([
      'launcher',
      'footer'
    ]);
  });
});
