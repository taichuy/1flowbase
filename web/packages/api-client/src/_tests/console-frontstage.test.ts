import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createFrontstageGroup,
  createFrontstagePage,
  createFrontstagePageTab,
  deleteFrontstagePageTab,
  deleteFrontstagePageNode,
  getFrontstageBlockCode,
  getFrontstagePageTabDetail,
  listFrontstagePageTabs,
  listFrontstagePages,
  moveFrontstagePageNode,
  saveFrontstageBlockCode,
  saveFrontstageTabDocument,
  updateFrontstagePageTab,
  updateFrontstagePageNodeTitle
} from '../console/frontstage';

describe('console-frontstage client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test.each([
    {
      name: 'page tree collection',
      request: () => listFrontstagePages('workspace-1'),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages',
        method: 'GET'
      }
    },
    {
      name: 'page tab collection',
      request: () => listFrontstagePageTabs('workspace-1', 'page-1'),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs',
        method: 'GET'
      }
    },
    {
      name: 'page tab detail by route segment',
      request: () =>
        getFrontstagePageTabDetail('workspace-1', 'page-1', 'analytics'),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/analytics',
        method: 'GET'
      }
    },
    {
      name: 'encoded JS block code ref',
      request: () =>
        getFrontstageBlockCode('workspace-1', 'page-1', 'hero/main'),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/block-codes/hero%2Fmain',
        method: 'GET'
      }
    }
  ])(
    'reads $name through the console frontstage route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );

  test.each([
    {
      name: 'group creation',
      request: () =>
        createFrontstageGroup(
          'workspace-1',
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
        path: '/api/console/frontstage/workspace-1/pages/groups',
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
          'workspace-1',
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
        path: '/api/console/frontstage/workspace-1/pages',
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
          'workspace-1',
          'page-1',
          { title: '页面-已重命名' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1',
        method: 'PATCH',
        body: { title: '页面-已重命名' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'metadata patch',
      request: () =>
        updateFrontstagePageNodeTitle(
          'workspace-1',
          'page-1',
          { tooltip: '展示在页面树', is_hidden: true },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1',
        method: 'PATCH',
        body: { tooltip: '展示在页面树', is_hidden: true },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'node move',
      request: () =>
        moveFrontstagePageNode(
          'workspace-1',
          'page-1',
          { parent_id: null, rank: '000000' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/move',
        method: 'POST',
        body: { parent_id: null, rank: '000000' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'node deletion',
      request: () =>
        deleteFrontstagePageNode('workspace-1', 'page-1', 'csrf-123'),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab creation',
      request: () =>
        createFrontstagePageTab(
          'workspace-1',
          'page-1',
          {
            title: 'Analytics',
            route_segment: 'analytics',
            rank: '002000'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs',
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
          'workspace-1',
          'page-1',
          'tab-1',
          { title: 'Renamed' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-1',
        method: 'PATCH',
        body: { title: 'Renamed' },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab deletion',
      request: () =>
        deleteFrontstagePageTab(
          'workspace-1',
          'page-1',
          'tab-1',
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tab document save',
      request: () =>
        saveFrontstageTabDocument(
          'workspace-1',
          'page-1',
          'tab-1',
          { payload: { version: 1, blocks: [{ id: 'hero-1' }] } },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-1/document',
        method: 'PUT',
        body: { payload: { version: 1, blocks: [{ id: 'hero-1' }] } },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'block code save',
      request: () =>
        saveFrontstageBlockCode(
          'workspace-1',
          'page-1',
          'hero',
          { code: 'export default function Hero() {}' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/frontstage/workspace-1/pages/page-1/block-codes/hero',
        method: 'PUT',
        body: { code: 'export default function Hero() {}' },
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
