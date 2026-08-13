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
      dependency_lock: [],
      tailwind_toolchain_lock: { package: 'tailwindcss', version: '4.3.3' },
      generated_css: 'a{}',
      generated_css_sha256: '5f546eb4606b5c2b7d2a449a5cc2bbb477ed5a246c7051ce871b12f2dbfc8419',
      compiler_identity: { name: 'tailwindcss', abi: 'v1' },
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
          source_code: "import 'tailwindcss'; export default Sales",
          dependency_lock: [],
          tailwind_toolchain_lock: { package: 'tailwindcss', version: '4.3.3' },
          generated_css: 'a{}',
          generated_css_sha256: '5f546eb4606b5c2b7d2a449a5cc2bbb477ed5a246c7051ce871b12f2dbfc8419',
          compiler_identity: { name: 'tailwindcss', abi: 'v1' }
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/workspace-1/pages/page-1/blocks/sales/code',
      method: 'PUT',
      body: expect.objectContaining({ source_code: expect.stringContaining('export default Sales') })
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

  test('exposes the runtime assembly as public root-to-target layers with frozen source', () => {
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
          source_code: 'export default function Root() {}',
          source_sha256: null,
          dependency_lock: null,
          tailwind_toolchain_lock: null,
          generated_css: null,
          generated_css_sha256: null,
          compiler_identity: null,
          executable_state: 'legacy'
        }
      ]
    } satisfies ConsoleFrontstageBlockRuntimeAssembly;

    expect(Object.keys(assembly.layers[0]).sort()).toEqual([
      'block_id',
      'compiler_identity',
      'dependency_lock',
      'executable_state',
      'generated_css',
      'generated_css_sha256',
      'input_mapping',
      'output_mapping',
      'parent_block_id',
      'presentation',
      'runtime_descriptor',
      'schema_version',
      'source_code',
      'source_sha256',
      'tab_id',
      'tailwind_toolchain_lock',
      'title'
    ]);
    expect(assembly.layers[0]).not.toHaveProperty('id');
    expect(assembly.layers[0]).not.toHaveProperty('code_ref');
    expect(assembly.layers[0]).not.toHaveProperty('workspace_id');
    expect(assembly.layers[0]).not.toHaveProperty('page_id');
  });
});
