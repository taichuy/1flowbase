import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../../transport';
import {
  createConsoleFrontstageBlockNode,
  deleteConsoleFrontstageBlockLeaf,
  deleteConsoleFrontstageBlockSubtree,
  getConsoleFrontstageBlockDeleteImpact,
  getConsoleFrontstageBlockNode,
  getConsoleFrontstageBlockNodeCode,
  listConsoleFrontstageBlockAncestors,
  listConsoleFrontstageBlockChildren,
  listConsoleFrontstageBlockDescendants,
  listConsoleFrontstageBlockRoots,
  moveConsoleFrontstageBlockNode,
  saveConsoleFrontstageBlockNodeCode,
  searchConsoleFrontstageBlocks,
  updateConsoleFrontstageBlockNode,
  type ConsoleFrontstageBlockNodeSummary
} from '../index';

describe('frontstage block tree client contract', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(transport, 'apiFetch').mockImplementation(
      async (input) => input as never
    );
  });

  test('AC-001/003 uses exact page-scoped read routes and snake_case queries', async () => {
    await expect(
      listConsoleFrontstageBlockRoots('workspace 1', 'page/1', { limit: 25 })
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks?limit=25',
      method: 'GET'
    });
    await expect(
      searchConsoleFrontstageBlocks('workspace 1', 'page/1', {
        query: 'sales drawer',
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/search?query=sales+drawer&limit=10',
      method: 'GET'
    });
    await expect(
      getConsoleFrontstageBlockNode('workspace 1', 'page/1', 'block/root')
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot',
      method: 'GET'
    });
    await expect(
      listConsoleFrontstageBlockChildren(
        'workspace 1',
        'page/1',
        'block/root',
        { limit: 30 }
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/children?limit=30'
    });
    await expect(
      listConsoleFrontstageBlockAncestors(
        'workspace 1',
        'page/1',
        'block/root'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/ancestors'
    });
    await expect(
      listConsoleFrontstageBlockDescendants(
        'workspace 1',
        'page/1',
        'block/root',
        { max_depth: 4, limit: 50 }
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/descendants?max_depth=4&limit=50'
    });
    await expect(
      getConsoleFrontstageBlockDeleteImpact(
        'workspace 1',
        'page/1',
        'block/root'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/delete-impact'
    });
    await expect(
      getConsoleFrontstageBlockNodeCode(
        'workspace 1',
        'page/1',
        'block/root'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/code',
      method: 'GET'
    });
  });

  test('AC-001/009 sends exact mutation methods and bodies', async () => {
    const createInput = {
      tab_id: 'tab-1',
      title: 'Sales',
      presentation: 'drawer' as const,
      parent_block_id: null,
      before_block_id: null,
      after_block_id: 'summary',
      code: 'export default function Sales() {}',
      input_mapping: { customer: 'page.customer' },
      output_mapping: { result: 'page.result' },
      runtime_descriptor: { kind: 'native_react' }
    };
    await expect(
      createConsoleFrontstageBlockNode(
        'workspace-1',
        'page-1',
        createInput,
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks',
      method: 'POST',
      body: createInput,
      csrfToken: 'csrf'
    });
    await expect(
      updateConsoleFrontstageBlockNode(
        'workspace-1',
        'page-1',
        'sales',
        { title: 'Sales details', presentation: 'page' },
        'csrf'
      )
    ).resolves.toMatchObject({
      method: 'PATCH',
      body: { title: 'Sales details', presentation: 'page' }
    });
    await expect(
      moveConsoleFrontstageBlockNode(
        'workspace-1',
        'page-1',
        'sales',
        {
          parent_block_id: 'reports',
          before_block_id: 'profit',
          after_block_id: null
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks/sales/move',
      method: 'POST',
      body: {
        parent_block_id: 'reports',
        before_block_id: 'profit',
        after_block_id: null
      }
    });
    await expect(
      deleteConsoleFrontstageBlockLeaf(
        'workspace-1',
        'page-1',
        'sales',
        'csrf'
      )
    ).resolves.toMatchObject({ method: 'DELETE', body: undefined });
    await expect(
      deleteConsoleFrontstageBlockSubtree(
        'workspace-1',
        'page-1',
        'reports',
        { expected_affected_count: 3 },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks/reports/delete-subtree',
      method: 'POST',
      body: { expected_affected_count: 3 }
    });
    await expect(
      saveConsoleFrontstageBlockNodeCode(
        'workspace-1',
        'page-1',
        'sales',
        { code: 'export default Sales' },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks/sales/code',
      method: 'PUT',
      body: { code: 'export default Sales' }
    });
  });

  test('AC-003 exposes only public block identity and backend response fields', () => {
    const summary = {
      block_id: 'sales',
      workspace_id: 'workspace-1',
      page_id: 'page-1',
      tab_id: 'tab-1',
      parent_block_id: null,
      rank: '001000',
      presentation: 'page',
      title: 'Sales',
      schema_version: 1,
      created_at: '2026-08-12T00:00:00Z',
      updated_at: '2026-08-12T00:00:00Z'
    } satisfies ConsoleFrontstageBlockNodeSummary;

    expect(summary.block_id).toBe('sales');
    expect(Object.keys(summary).sort()).toEqual([
      'block_id',
      'created_at',
      'page_id',
      'parent_block_id',
      'presentation',
      'rank',
      'schema_version',
      'tab_id',
      'title',
      'updated_at',
      'workspace_id'
    ]);
  });
});
