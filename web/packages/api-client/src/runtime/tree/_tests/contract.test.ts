import { describe, expect, test, vi } from 'vitest';
vi.mock('../../../transport', () => ({
  apiFetch: vi.fn(async (input) => input)
}));
import {
  listRuntimeTreeRoots,
  listRuntimeTreeChildren,
  listRuntimeTreeAncestors,
  listRuntimeTreeDescendants,
  searchRuntimeTree
} from '../index';
describe('runtime tree cursor contract', () => {
  test('encodes model, IDs and opaque cursors without reinterpreting them', async () => {
    await expect(
      listRuntimeTreeRoots('org model', { limit: 10, cursor: 'a+/=' })
    ).resolves.toMatchObject({
      path: '/api/runtime/models/org%20model/tree/roots?limit=10&cursor=a%2B%2F%3D'
    });
    await expect(
      listRuntimeTreeChildren('org', 'id/a', { limit: 2 })
    ).resolves.toMatchObject({
      path: '/api/runtime/models/org/tree/children/id%2Fa?limit=2'
    });
    await expect(
      listRuntimeTreeAncestors('org', 'child')
    ).resolves.toMatchObject({
      path: '/api/runtime/models/org/tree/ancestors/child'
    });
    await expect(
      listRuntimeTreeDescendants('org', 'root', {
        max_depth: 3,
        limit: 4,
        cursor: 'next',
        include_path: true
      })
    ).resolves.toMatchObject({
      path: '/api/runtime/models/org/tree/descendants/root?max_depth=3&limit=4&cursor=next&include_path=true'
    });
    await expect(
      searchRuntimeTree('org', { prefix: '研发', limit: 5, cursor: 'next' })
    ).resolves.toMatchObject({
      path: '/api/runtime/models/org/tree/search?prefix=%E7%A0%94%E5%8F%91&limit=5&cursor=next'
    });
  });
});
