import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../../transport';
import {
  createConsoleFrontstageBlockNode,
  deleteConsoleFrontstageBlockLeaf,
  deleteConsoleFrontstageBlockSubtree,
  getConsoleFrontstageBlockDeleteImpact,
  getConsoleFrontstageBlockNode,
  getConsoleFrontstageBlockNodeCode,
  getConsoleFrontstageBlockRuntimeAssembly,
  listConsoleFrontstageBlockAncestors,
  listConsoleFrontstageBlockChildren,
  listConsoleFrontstageBlockDescendants,
  listConsoleFrontstageBlockRoots,
  moveConsoleFrontstageBlockNode,
  openConsoleFrontstageBlock,
  saveConsoleFrontstageBlockNodeCode,
  searchConsoleFrontstageBlocks,
  updateConsoleFrontstageBlockDescriptors,
  updateConsoleFrontstageBlockNode,
  type ConsoleFrontstageBlockNodeSummary,
  type ConsoleFrontstageBlockRuntimeAssembly
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
      listConsoleFrontstageBlockRoots('workspace 1', 'page/1', {
        tab_id: 'tab/1',
        limit: 25
      })
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks?tab_id=tab%2F1&limit=25',
      method: 'GET'
    });
    await expect(
      searchConsoleFrontstageBlocks('workspace 1', 'page/1', {
        tab_id: 'tab/1',
        query: 'sales drawer',
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/search?tab_id=tab%2F1&query=sales+drawer&limit=10',
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
      listConsoleFrontstageBlockAncestors('workspace 1', 'page/1', 'block/root')
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
      getConsoleFrontstageBlockNodeCode('workspace 1', 'page/1', 'block/root')
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/code',
      method: 'GET'
    });
    await expect(
      getConsoleFrontstageBlockRuntimeAssembly(
        'workspace 1',
        'page/1',
        'block/root'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/runtime-assembly',
      method: 'GET'
    });
    await expect(
      openConsoleFrontstageBlock('workspace 1', 'page/1', 'block/root')
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace%201/pages/page%2F1/blocks/block%2Froot/open',
      method: 'GET'
    });
  });

  test('AC-001/009 sends exact mutation methods and bodies', async () => {
    const createInput = {
      tab_id: 'tab-1',
      title: 'Sales',
      description: 'Sales block',
      presentation: 'drawer' as const,
      parent_block_id: null,
      before_block_id: null,
      after_block_id: 'summary',
      source_code: "import 'tailwindcss'; export default function Sales() {}",
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
        {
          title: 'Sales details',
          description: 'Sales details block',
          presentation: 'page'
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      method: 'PATCH',
      body: {
        title: 'Sales details',
        description: 'Sales details block',
        presentation: 'page'
      }
    });
    await expect(
      updateConsoleFrontstageBlockDescriptors(
        'workspace-1',
        'page-1',
        'tab-1',
        {
          updates: [
            {
              block_id: 'sales',
              runtime_descriptor: { 'x-layout': { order: 1 } }
            }
          ]
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-1/block-descriptors',
      method: 'PUT',
      csrfToken: 'csrf'
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
    const leafDelete = await deleteConsoleFrontstageBlockLeaf(
      'workspace-1',
      'page-1',
      'sales',
      'csrf'
    );
    expect(leafDelete).toMatchObject({ method: 'DELETE' });
    expect(leafDelete).not.toHaveProperty('body');
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
        {
          source_code: "import 'tailwindcss'; export default Sales"
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks/sales/code',
      method: 'PUT',
      body: expect.objectContaining({
        source_code: expect.stringContaining('export default Sales')
      })
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
      description: 'Sales block',
      schema_version: 1,
      created_at: '2026-08-12T00:00:00Z',
      updated_at: '2026-08-12T00:00:00Z'
    } satisfies ConsoleFrontstageBlockNodeSummary;

    expect(summary.block_id).toBe('sales');
    expect(Object.keys(summary).sort()).toEqual([
      'block_id',
      'created_at',
      'description',
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

  test('exposes runtime assembly as lightweight root-to-target source references', () => {
    const assembly = {
      layers: [
        {
          block_id: 'root',
          tab_id: 'tab-1',
          parent_block_id: null,
          title: 'Root',
          presentation: 'page',
          schema_version: 1,
          input_mapping: {},
          output_mapping: {},
          runtime_descriptor: { rendererVersion: 'v1' },
          code_ref: 'frontstage.block.root',
          source_revision: null
        }
      ]
    } satisfies ConsoleFrontstageBlockRuntimeAssembly;

    expect(Object.keys(assembly.layers[0]).sort()).toEqual([
      'block_id',
      'code_ref',
      'input_mapping',
      'output_mapping',
      'parent_block_id',
      'presentation',
      'runtime_descriptor',
      'schema_version',
      'source_revision',
      'tab_id',
      'title'
    ]);
    expect(assembly.layers[0]).not.toHaveProperty('id');
    expect(assembly.layers[0]).not.toHaveProperty('source_code');
    expect(assembly.layers[0]).not.toHaveProperty('workspace_id');
    expect(assembly.layers[0]).not.toHaveProperty('page_id');
  });
});
