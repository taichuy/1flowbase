import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createFrontstageGroup,
  createFrontstagePage,
  createFrontstagePageTab,
  deleteFrontstagePageTab,
  deleteFrontstagePageNode,
  dispatchFrontstageCallable,
  dispatchFrontstageCallableStream,
  type FrontstageCallableBinaryResource,
  getFrontstageInterfaceCapability,
  getFrontstageComponent,
  resolveFrontstageComponentDependencyLock,
  getFrontstagePageTabDetail,
  frontstageComponentModuleAssetPath,
  listFrontstagePageTabs,
  listFrontstageInterfaceCapabilities,
  listFrontstageComponents,
  listFrontstagePages,
  moveFrontstagePageNode,
  saveFrontstageTabDocument,
  type ConsoleFrontstageComponent,
  updateFrontstagePageTab,
  updateFrontstagePageNodeTitle
} from '../console/frontstage';

describe('console-frontstage client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchResource').mockImplementation(async (input) =>
    (input.body as { path?: string } | undefined)?.path ===
    '/api/console/export-logs'
      ? {
          kind: 'blob',
          blob: new Blob([new Uint8Array([1, 2, 3])]),
          filename: 'export.zip',
          contentType: 'application/zip'
        }
      : (input.body as { path?: string } | undefined)?.path ===
          '/api/console/delete-tab'
        ? { kind: 'no_content' }
        : { kind: 'json', value: input as never }
  );
  vi.spyOn(transport, 'apiFetchStream').mockResolvedValue({
    body: new ReadableStream({
      start(controller) {
        controller.enqueue(
          new TextEncoder().encode('data: {"progress":1}\n\ndata: complete\n\n')
        );
        controller.close();
      }
    }),
    cancel: vi.fn()
  });

  test('WP-D4 exposes the persisted raw component record contract', () => {
    const component = {
      id: '019c0000-0000-7000-8000-000000000001',
      scope_id: '00000000-0000-0000-0000-000000000000',
      component_code: 'official.surface',
      name: 'Surface',
      description: 'Native React surface with standard DOM props.',
      import_code: "import { Surface } from '@definitely/not-installed';",
      source_code: '<Surface className="card">Content</Surface>',
      origin: 'official',
      source: 'official',
      group: 'layout',
      upstream: { identity: '@definitely/not-installed', version: '99.0.0' },
      version: '1.0.0',
      keywords: ['surface'],
      catalog_updated_at: null,
      source_locator: null,
      source_checksum: null,
      created_at: '2026-08-23T00:00:00Z',
      updated_at: '2026-08-23T00:00:00Z'
    } satisfies ConsoleFrontstageComponent;

    expect(component).toMatchObject({
      component_code: 'official.surface',
      import_code: "import { Surface } from '@definitely/not-installed';",
      source_code: '<Surface className="card">Content</Surface>'
    });
  });

  test('D2-AC-001 builds the auth-scoped component module asset route without a workspace URL segment', () => {
    expect(
      frontstageComponentModuleAssetPath(
        'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
      )
    ).toBe(
      '/api/console/frontstage/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
    );
  });

  test.each([
    {
      name: 'OpenAPI capability catalog',
      request: () =>
        listFrontstageInterfaceCapabilities({
          path_prefixes: ['/api/public/', '/api/console/settings/auth-center/'],
          path_query: '/api/console/applications',
          adapter_id: 'console_openapi',
          method: 'GET',
          offset: 20,
          limit: 20
        }),
      expected: {
        path: '/api/console/frontstage/interface-capabilities?path_prefixes=%2Fapi%2Fpublic%2F%2C%2Fapi%2Fconsole%2Fsettings%2Fauth-center%2F&path_query=%2Fapi%2Fconsole%2Fapplications&adapter_id=console_openapi&method=GET&offset=20&limit=20',
        method: 'GET'
      }
    },
    {
      name: 'persisted component catalog',
      request: () =>
        listFrontstageComponents({
          query: 'button',
          offset: 20,
          limit: 20
        }),
      expected: {
        path: '/api/console/frontstage/components?query=button&offset=20&limit=20',
        method: 'GET'
      }
    },
    {
      name: 'page tree collection',
      request: () => listFrontstagePages(),
      expected: {
        path: '/api/console/frontstage/pages',
        method: 'GET'
      }
    },
    {
      name: 'page tab collection',
      request: () => listFrontstagePageTabs('page-1'),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs',
        method: 'GET'
      }
    },
    {
      name: 'page tab detail by route segment',
      request: () => getFrontstagePageTabDetail('page-1', 'analytics'),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs/analytics',
        method: 'GET'
      }
    }
  ])(
    'reads $name through the console frontstage route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );

  test('loads one encoded interface capability detail on demand', async () => {
    await expect(
      getFrontstageInterfaceCapability('published/interface:detail')
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/interface-capabilities/published%2Finterface%3Adetail',
      method: 'GET'
    });
  });

  test('loads one persisted component record detail on demand', async () => {
    await expect(
      getFrontstageComponent('019c0000-0000-7000-8000-000000000001')
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/components/019c0000-0000-7000-8000-000000000001',
      method: 'GET'
    });
  });

  test('resolves a component lock from the current block source', async () => {
    await expect(
      resolveFrontstageComponentDependencyLock(
        "import { Chart } from '@acme/charts';"
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/component-dependency-lock',
      method: 'POST',
      body: { source_code: "import { Chart } from '@acme/charts';" }
    });
  });

  test('dispatches a source-described callable through the page-tab scope', async () => {
    await expect(
      dispatchFrontstageCallable(
        'page-1',
        'tab-1',
        {
          block_id: 'block-1',
          method: 'GET',
          path: '/api/console/conversations',
          request: { query: { filter: 'status=active' } }
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/frontstage/pages/page-1/tabs/tab-1/callable-interfaces/dispatch',
      method: 'POST',
      body: {
        block_id: 'block-1',
        method: 'GET',
        path: '/api/console/conversations',
        request: { query: { filter: 'status=active' } }
      },
      csrfToken: 'csrf-123'
    });
  });

  test('preserves controlled binary responses as a worker-safe byte resource', async () => {
    await expect(
      dispatchFrontstageCallable<FrontstageCallableBinaryResource>(
        'page-1',
        'tab-1',
        {
          block_id: 'block-1',
          method: 'GET',
          path: '/api/console/export-logs'
        },
        'csrf-123'
      )
    ).resolves.toEqual({
      bytes: new Uint8Array([1, 2, 3]),
      file_name: 'export.zip',
      content_type: 'application/zip'
    });
    expect(transport.apiFetchResource).toHaveBeenCalledWith(
      expect.objectContaining({
        method: 'POST',
        csrfToken: 'csrf-123'
      })
    );
  });

  test('maps a 204 callable response to undefined', async () => {
    await expect(
      dispatchFrontstageCallable(
        'page-1',
        'tab-1',
        {
          block_id: 'block-1',
          method: 'DELETE',
          path: '/api/console/delete-tab'
        },
        'csrf-123'
      )
    ).resolves.toBeUndefined();
  });

  test('parses callable SSE data through a cancellable async iterable', async () => {
    const iterable = await dispatchFrontstageCallableStream<
      { progress: number } | string
    >(
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        method: 'GET',
        path: '/api/console/watch-run'
      },
      'csrf-123'
    );
    const events = [];
    for await (const event of iterable) events.push(event);
    expect(events).toEqual([{ progress: 1 }, 'complete']);
  });

  test.each([
    {
      name: 'group creation',
      request: () =>
        createFrontstageGroup(
          {
            title: '分组 1',
            icon: 'FolderOutlined',
            tooltip: '分组描述',
            parent_id: null,
            rank: '001000',
            placement: 'topbar',
            slug: 'sales-space'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/groups',
        method: 'POST',
        body: {
          title: '分组 1',
          icon: 'FolderOutlined',
          tooltip: '分组描述',
          parent_id: null,
          rank: '001000',
          placement: 'topbar',
          slug: 'sales-space'
        },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'page creation',
      request: () =>
        createFrontstagePage(
          {
            title: '页面 新建 1',
            icon: 'FileTextOutlined',
            tooltip: '页面描述',
            parent_id: 'group-1',
            rank: '002000',
            placement: 'topbar'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages',
        method: 'POST',
        body: {
          title: '页面 新建 1',
          icon: 'FileTextOutlined',
          tooltip: '页面描述',
          parent_id: 'group-1',
          rank: '002000',
          placement: 'topbar'
        },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'title patch',
      request: () =>
        updateFrontstagePageNodeTitle(
          'page-1',
          { title: '页面-已重命名' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1',
        method: 'PATCH',
        body: { title: '页面-已重命名' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'metadata patch',
      request: () =>
        updateFrontstagePageNodeTitle(
          'page-1',
          { tooltip: '展示在页面树', is_hidden: true },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1',
        method: 'PATCH',
        body: { tooltip: '展示在页面树', is_hidden: true },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'node move',
      request: () =>
        moveFrontstagePageNode(
          'page-1',
          { parent_id: null, rank: '000000' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1/move',
        method: 'POST',
        body: { parent_id: null, rank: '000000' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'node deletion',
      request: () => deleteFrontstagePageNode('page-1', 'csrf-123'),
      expected: {
        path: '/api/console/frontstage/pages/page-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab creation',
      request: () =>
        createFrontstagePageTab(
          'page-1',
          {
            title: 'Analytics',
            route_segment: 'analytics',
            rank: '002000'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs',
        method: 'POST',
        body: {
          title: 'Analytics',
          route_segment: 'analytics',
          rank: '002000'
        },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab metadata patch',
      request: () =>
        updateFrontstagePageTab(
          'page-1',
          'tab-1',
          { title: 'Renamed' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs/tab-1',
        method: 'PATCH',
        body: { title: 'Renamed' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab deletion',
      request: () => deleteFrontstagePageTab('page-1', 'tab-1', 'csrf-123'),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs/tab-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab document save',
      request: () =>
        saveFrontstageTabDocument(
          'page-1',
          'tab-1',
          { payload: { version: 1, 'x-layout-mode': 'auto' } },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/pages/page-1/tabs/tab-1/document',
        method: 'PUT',
        body: { payload: { version: 1, 'x-layout-mode': 'auto' } },
        csrfToken: 'csrf-123'
      }
    }
  ])(
    'writes $name through the console frontstage route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );
});
