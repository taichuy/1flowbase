import { describe, expect, test, vi } from 'vitest';
import * as apiClient from '@1flowbase/api-client';

import {
  fetchFrontstageBlockCode,
  frontstageBlockCodeQueryKey,
  saveFrontstageBlockCode
} from '../api/block-code';
import {
  fetchFrontstageBlockCatalog,
  frontstageBlockCatalogQueryKey,
  frontstageBlockCatalogQueryKeyPrefix
} from '../api/block-catalog';
import {
  fetchFrontstagePageContent,
  frontstagePageContentQueryKey,
  saveFrontstagePageContent
} from '../api/page-content';
import {
  createFrontstagePageGroupNode,
  createFrontstagePageNode,
  deleteFrontstageNode,
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  moveFrontstageNode,
  renameFrontstagePageNode,
  updateFrontstagePageNodeMetadata
} from '../api/page-tree';

describe('frontstage page tree feature api', () => {
  test('uses a workspace-scoped page tree query key', () => {
    expect(frontstagePageTreeQueryKey('workspace-1')).toEqual([
      'frontstage',
      'workspace-1',
      'page-tree'
    ]);
  });

  test('adapts page tree read and write calls to api-client DTOs', async () => {
    const listSpy = vi
      .spyOn(apiClient, 'listFrontstagePages')
      .mockResolvedValue([]);
    const createGroupSpy = vi
      .spyOn(apiClient, 'createFrontstageGroup')
      .mockResolvedValue({
        id: 'group-1',
        title: '分组 1',
        icon: null,
        tooltip: null,
        is_hidden: false,
        placement: 'sidebar',
        content_presentation: 'single',
        slug: null,
        kind: 'group',
        parent_id: null,
        rank: '001000'
      });
    const createPageSpy = vi
      .spyOn(apiClient, 'createFrontstagePage')
      .mockResolvedValue({
        page: {
          id: 'page-1',
          title: '页面 1',
          icon: null,
          tooltip: null,
          is_hidden: false,
          placement: 'sidebar',
          content_presentation: 'single',
          slug: null,
          kind: 'page',
          parent_id: 'group-1',
          rank: '001000'
        },
        default_tab: {
          id: 'tab-1',
          page_id: 'page-1',
          title: null,
          rank: 'a',
          is_default: true,
          route_segment: null,
          document_root_uid: 'frontstage.tab.1.root'
        }
      });
    const updateSpy = vi
      .spyOn(apiClient, 'updateFrontstagePageNodeTitle')
      .mockResolvedValue({
        id: 'page-1',
        title: '页面 新名',
        icon: null,
        tooltip: null,
        is_hidden: false,
        placement: 'sidebar',
        content_presentation: 'single',
        slug: null,
        kind: 'page',
        parent_id: null,
        rank: '001000'
      });
    const moveSpy = vi
      .spyOn(apiClient, 'moveFrontstagePageNode')
      .mockResolvedValue({
        id: 'page-1',
        title: '页面 新名',
        icon: null,
        tooltip: null,
        is_hidden: false,
        placement: 'sidebar',
        content_presentation: 'single',
        slug: null,
        kind: 'page',
        parent_id: null,
        rank: '000000'
      });
    const deleteSpy = vi
      .spyOn(apiClient, 'deleteFrontstagePageNode')
      .mockResolvedValue(undefined);

    try {
      await fetchFrontstagePageTree('workspace-1');
      await createFrontstagePageGroupNode(
        'workspace-1',
        {
          title: '分组 1',
          icon: 'FolderOutlined',
          tooltip: '分组描述',
          parentId: null,
          rank: '001000'
        },
        'csrf-123'
      );
      await createFrontstagePageNode(
        'workspace-1',
        {
          title: '页面 1',
          icon: 'FileTextOutlined',
          tooltip: '页面描述',
          parentId: 'group-1',
          rank: '001000'
        },
        'csrf-123'
      );
      await renameFrontstagePageNode(
        'workspace-1',
        'page-1',
        { title: '页面 新名' },
        'csrf-123'
      );
      await updateFrontstagePageNodeMetadata(
        'workspace-1',
        'page-1',
        { tooltip: '展示在页面树', isHidden: true },
        'csrf-123'
      );
      await moveFrontstageNode(
        'workspace-1',
        'page-1',
        { parentId: null, rank: '000000' },
        'csrf-123'
      );
      await deleteFrontstageNode('workspace-1', 'page-1', 'csrf-123');

      expect(listSpy).toHaveBeenCalledWith('workspace-1', expect.any(String));
      expect(createGroupSpy).toHaveBeenCalledWith(
        'workspace-1',
        {
          title: '分组 1',
          icon: 'FolderOutlined',
          tooltip: '分组描述',
          parent_id: null,
          rank: '001000'
        },
        'csrf-123',
        expect.any(String)
      );
      expect(createPageSpy).toHaveBeenCalledWith(
        'workspace-1',
        {
          title: '页面 1',
          icon: 'FileTextOutlined',
          tooltip: '页面描述',
          parent_id: 'group-1',
          rank: '001000'
        },
        'csrf-123',
        expect.any(String)
      );
      expect(updateSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { title: '页面 新名' },
        'csrf-123',
        expect.any(String)
      );
      expect(updateSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { tooltip: '展示在页面树', is_hidden: true },
        'csrf-123',
        expect.any(String)
      );
      expect(moveSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { parent_id: null, rank: '000000' },
        'csrf-123',
        expect.any(String)
      );
      expect(deleteSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'csrf-123',
        expect.any(String)
      );
    } finally {
      listSpy.mockRestore();
      createGroupSpy.mockRestore();
      createPageSpy.mockRestore();
      updateSpy.mockRestore();
      moveSpy.mockRestore();
      deleteSpy.mockRestore();
    }
  });
});

describe('frontstage page content feature api', () => {
  test('uses a workspace and page scoped detail query key', () => {
    expect(frontstagePageContentQueryKey('workspace-1', 'page-1', 'tab-1')).toEqual([
      'frontstage',
      'workspace-1',
      'pages',
      'page-1',
      'tabs',
      'tab-1',
      'content'
    ]);
  });

  test('adapts page detail DTOs to camelCase output', async () => {
    const detailSpy = vi
      .spyOn(apiClient, 'getFrontstagePageTabDetail')
      .mockResolvedValue({
        page: {
          id: 'page-1',
          title: '页面 1',
          icon: null,
          tooltip: null,
          is_hidden: false,
          placement: 'sidebar',
          content_presentation: 'tabs',
          slug: null,
          kind: 'page',
          parent_id: 'group-1',
          rank: '001000'
        },
        tab: {
          id: 'tab-1',
          page_id: 'page-1',
          title: '概览',
          rank: '001000',
          is_default: true,
          route_segment: null,
          document_root_uid: 'root-1'
        },
        document: {
          root_uid: 'root-1',
          payload: { blocks: [] }
        }
      });

    try {
      await expect(
        fetchFrontstagePageContent('workspace-1', 'page-1', 'tab-1')
      ).resolves.toEqual({
        page: {
          id: 'page-1',
          title: '页面 1',
          icon: null,
          tooltip: null,
          kind: 'page',
          parentId: 'group-1',
          rank: '001000',
          contentPresentation: 'tabs'
        },
        tab: {
          id: 'tab-1',
          pageId: 'page-1',
          title: '概览',
          rank: '001000',
          isDefault: true,
          routeSegment: null,
          documentRootUid: 'root-1'
        },
        document: {
          rootUid: 'root-1',
          payload: { blocks: [] }
        }
      });
      expect(detailSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-1',
        expect.any(String)
      );
    } finally {
      detailSpy.mockRestore();
    }
  });

  test('adapts page content save calls to api-client DTOs', async () => {
    const saveSpy = vi
      .spyOn(apiClient, 'saveFrontstageTabDocument')
      .mockResolvedValue({
        page: {
          id: 'page-1',
          title: '页面 1',
          icon: null,
          tooltip: null,
          is_hidden: false,
          placement: 'sidebar',
          content_presentation: 'single',
          slug: null,
          kind: 'page',
          parent_id: 'group-1',
          rank: '001000'
        },
        tab: {
          id: 'tab-1',
          page_id: 'page-1',
          title: '概览',
          rank: '001000',
          is_default: true,
          route_segment: null,
          document_root_uid: 'root-1'
        },
        document: {
          root_uid: 'root-1',
          payload: {
            version: 1,
            blocks: [{ id: 'hero-1', renderer_version: 'v1' }]
          }
        }
      });

    try {
      await expect(
        saveFrontstagePageContent(
          'workspace-1',
          'page-1',
          'tab-1',
          {
            payload: {
              version: 1,
              blocks: [{ id: 'hero-1', renderer_version: 'v1' }]
            }
          },
          'csrf-123'
        )
      ).resolves.toEqual({
        page: {
          id: 'page-1',
          title: '页面 1',
          icon: null,
          tooltip: null,
          kind: 'page',
          parentId: 'group-1',
          rank: '001000',
          contentPresentation: 'single'
        },
        tab: {
          id: 'tab-1',
          pageId: 'page-1',
          title: '概览',
          rank: '001000',
          isDefault: true,
          routeSegment: null,
          documentRootUid: 'root-1'
        },
        document: {
          rootUid: 'root-1',
          payload: {
            version: 1,
            blocks: [{ id: 'hero-1', renderer_version: 'v1' }]
          }
        }
      });
      expect(saveSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-1',
        {
          payload: {
            version: 1,
            blocks: [{ id: 'hero-1', renderer_version: 'v1' }]
          }
        },
        'csrf-123',
        expect.any(String)
      );
    } finally {
      saveSpy.mockRestore();
    }
  });
});

describe('frontstage block code feature api', () => {
  test('uses an actor, workspace, page, and codeRef scoped query key', () => {
    expect(
      frontstageBlockCodeQueryKey(
        'workspace-1',
        'page-1',
        'hero',
        'actor-1'
      )
    ).toEqual([
      'frontstage',
      'actor-1',
      'workspace-1',
      'pages',
      'page-1',
      'block-code',
      'hero'
    ]);
  });

  test('adapts block code calls while preserving the source hash field', async () => {
    const readSpy = vi
      .spyOn(apiClient, 'getFrontstageBlockCode')
      .mockResolvedValue({
        page_id: 'page-1',
        code_ref: 'hero',
        code: 'export default 1;',
        source_sha256: 'read-source-sha256'
      });
    const saveSpy = vi
      .spyOn(apiClient, 'saveFrontstageBlockCode')
      .mockResolvedValue({
        page_id: 'page-1',
        code_ref: 'hero',
        code: 'export default 2;',
        source_sha256: 'saved-source-sha256'
      });

    try {
      await expect(
        fetchFrontstageBlockCode('workspace-1', 'page-1', 'hero')
      ).resolves.toEqual({
        pageId: 'page-1',
        codeRef: 'hero',
        code: 'export default 1;',
        source_sha256: 'read-source-sha256'
      });
      await expect(
        saveFrontstageBlockCode(
          'workspace-1',
          'page-1',
          { codeRef: 'hero', code: 'export default 2;' },
          'csrf-123'
        )
      ).resolves.toEqual({
        pageId: 'page-1',
        codeRef: 'hero',
        code: 'export default 2;',
        source_sha256: 'saved-source-sha256'
      });
      expect(readSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'hero',
        expect.any(String)
      );
      expect(saveSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'hero',
        { code: 'export default 2;' },
        'csrf-123',
        expect.any(String)
      );
    } finally {
      readSpy.mockRestore();
      saveSpy.mockRestore();
    }
  });
});

describe('frontstage block catalog feature api', () => {
  test('isolates block catalog keys by workspace, actor, and permissions', () => {
    expect(frontstageBlockCatalogQueryKeyPrefix).toEqual([
      'frontstage',
      'block-catalog'
    ]);
    expect(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    ).toEqual([
      'frontstage',
      'block-catalog',
      'workspace-1',
      'actor-1',
      'role:member|permissions:frontstage.page.design'
    ]);
    expect(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-2',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    ).not.toEqual(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    );
    expect(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-2',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    ).not.toEqual(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    );
    expect(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:'
      })
    ).not.toEqual(
      frontstageBlockCatalogQueryKey({
        workspaceId: 'workspace-1',
        actorId: 'actor-1',
        permissionFingerprint: 'role:member|permissions:frontstage.page.design'
      })
    );
  });

  test('adapts block catalog reads to api-client DTOs', async () => {
    const listSpy = vi
      .spyOn(apiClient, 'listConsoleFrontendBlocks')
      .mockResolvedValue([
        {
          installation_id: 'installation-1',
          provider_code: 'official',
          plugin_id: 'official.blocks',
          plugin_version: '1.0.0',
          contribution_code: 'official.hero',
          title: 'Hero',
          runtime: 'iframe',
          entry: 'blocks/hero.html',
          context_contract: {
            primitives: ['record'],
            input_schema: {
              type: 'object',
              properties: {
                title: { type: 'string' }
              }
            }
          },
          permissions: {
            network: 'deny',
            storage: 'read',
            secrets: 'deny'
          },
          ui_capabilities: ['resizable', 'configure']
        }
      ]);

    try {
      await expect(fetchFrontstageBlockCatalog()).resolves.toEqual([
        {
          installation_id: 'installation-1',
          provider_code: 'official',
          plugin_id: 'official.blocks',
          plugin_version: '1.0.0',
          contribution_code: 'official.hero',
          title: 'Hero',
          runtime: 'iframe',
          entry: 'blocks/hero.html',
          context_contract: {
            primitives: ['record'],
            input_schema: {
              type: 'object',
              properties: {
                title: { type: 'string' }
              }
            }
          },
          permissions: {
            network: 'deny',
            storage: 'read',
            secrets: 'deny'
          },
          ui_capabilities: ['resizable', 'configure']
        }
      ]);
      expect(listSpy).toHaveBeenCalledWith(expect.any(String));
    } finally {
      listSpy.mockRestore();
    }
  });
});
