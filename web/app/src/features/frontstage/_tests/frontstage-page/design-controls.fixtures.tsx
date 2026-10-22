import { createNativePreparationSource } from '../page-canvas/fixtures/native-preparation-source';
import { act, fireEvent, render, screen } from '@testing-library/react';
import type { ConsoleFrontstageBlockNode } from '@1flowbase/api-client';
import { useState } from 'react';
import { beforeAll, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../../../state/frontstage-design-mode-store';
import type {
  FrontstagePageContent,
  SaveFrontstageTabDocumentInput
} from '../../api/page-content';

import {
  createFrontstagePageContentFixture,
  type FrontstagePageContentFixtureOverrides
} from '../frontstage-page-content-fixtures';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstagePageTab } from '../../api/page-tabs';
import {
  insertPageIntoGroup,
  moveNodeInTree,
  removeNodeFromTree,
  renameNodeInTree
} from '../../lib/page-tree';
import { FrontStagePage } from '../../pages/FrontStagePage';

const pageContentSaveHook = vi.hoisted(() => ({
  useFrontstagePageContentSave: vi.fn()
}));
const blockCatalogHook = vi.hoisted(() => ({
  useFrontstageBlockCatalog: vi.fn()
}));
const runtimeSessionsHook = vi.hoisted(() => ({
  useFrontstagePageCanvasNativePreparations: vi.fn(() => ({
    preparations: createNativePreparationSource([]),
    retryBlock: vi.fn()
  }))
}));
const blockTreeApi = vi.hoisted(() => ({
  createFrontstageBlockNode: vi.fn(),
  updateFrontstageBlockDescriptors: vi.fn(),
  updateFrontstageBlockNode: vi.fn(),
  fetchFrontstageBlockNode: vi.fn(),
  fetchFrontstageBlockNodeCode: vi.fn(),
  saveFrontstageBlockNodeCode: vi.fn(),
  frontstageBlockTreeQueryKeys: {
    block: (workspaceId: string, pageId: string, blockId: string) => [
      'frontstage',
      workspaceId,
      pageId,
      blockId,
      'detail'
    ],
    code: (workspaceId: string, pageId: string, blockId: string) => [
      'frontstage',
      workspaceId,
      pageId,
      blockId,
      'code'
    ]
  }
}));
const pageTabsApi = vi.hoisted(() => ({
  createFrontstagePageTab: vi.fn(),
  deleteFrontstagePageTab: vi.fn(),
  fetchFrontstagePageTabs: vi.fn(),
  moveFrontstagePageTab: vi.fn(),
  renameFrontstagePageTab: vi.fn(),
  frontstagePageTabsQueryKey: vi.fn((workspaceId: string, pageId: string) => [
    'frontstage',
    workspaceId,
    'pages',
    pageId,
    'tabs'
  ])
}));
const trialPanel = vi.hoisted(() => ({
  render: vi.fn()
}));
const componentCapabilitiesApi = vi.hoisted(() => ({
  resolveFrontstageComponentDependencyLock: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-page-content-save',
  () => pageContentSaveHook
);
vi.mock('../../hooks/use-frontstage-block-catalog', () => blockCatalogHook);
vi.mock(
  '../../hooks/use-frontstage-page-canvas-native-preparations',
  () => runtimeSessionsHook
);
vi.mock('../../api/block-tree', () => blockTreeApi);
vi.mock('../../api/page-tabs', () => pageTabsApi);
vi.mock('../../api/components', async () => {
  const actual = await vi.importActual<typeof import('../../api/components')>(
    '../../api/components'
  );
  return {
    ...actual,
    resolveFrontstageComponentDependencyLock:
      componentCapabilitiesApi.resolveFrontstageComponentDependencyLock
  };
});
vi.mock('../../components/jsx-studio/JsxStudioRunPanel', () => ({
  JsxStudioRunPanel: (props: unknown) => {
    trialPanel.render(props);
    return <div data-testid="captured-jsx-studio-run-panel" />;
  }
}));

const SLOW_FRONTSTAGE_TEST_TIMEOUT = 20_000;
const PLUGIN_CODE_TEMPLATE = `
export default function PluginBlock() {
  return <p>Plugin template ready</p>;
}
`.trim();

vi.setConfig({ testTimeout: SLOW_FRONTSTAGE_TEST_TIMEOUT });

beforeAll(async () => {
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
});

type TestFrontStageTreeNode = {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  kind: 'group' | 'page';
  children?: TestFrontStageTreeNode[];
};

type FrontstagePageContentSaveState = {
  save: ReturnType<typeof vi.fn>;
  saving: boolean;
  isPending: boolean;
  error: Error | null;
  reset: ReturnType<typeof vi.fn>;
  clearError: ReturnType<typeof vi.fn>;
};

function authenticate(permissions: string[]) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'actor-1',
      account: 'normal-user',
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'normal-user',
      email: 'user@example.com',
      phone: null,
      nickname: 'Normal User',
      name: 'Normal User',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'developer',
      permissions
    }
  });
}

function createBackendPage(pageId: string): TestFrontStageTreeNode {
  return {
    id: pageId,
    title: `页面 ${pageId}`,
    kind: 'page'
  };
}

function updateNodeMetadataInTree(
  nodes: TestFrontStageTreeNode[],
  nodeId: string,
  input: { icon?: string | null; tooltip?: string | null; isHidden?: boolean }
): TestFrontStageTreeNode[] {
  return nodes.map((node) => {
    const nextNode =
      node.id === nodeId
        ? {
            ...node,
            icon: Object.prototype.hasOwnProperty.call(input, 'icon')
              ? input.icon
              : node.icon,
            tooltip: Object.prototype.hasOwnProperty.call(input, 'tooltip')
              ? input.tooltip
              : node.tooltip,
            is_hidden: Object.prototype.hasOwnProperty.call(input, 'isHidden')
              ? input.isHidden
              : node.is_hidden
          }
        : node;

    return {
      ...nextNode,
      children: nextNode.children
        ? updateNodeMetadataInTree(nextNode.children, nodeId, input)
        : nextNode.children
    };
  });
}

function createPageContent(
  overrides: FrontstagePageContentFixtureOverrides = {}
): FrontstagePageContent {
  return createFrontstagePageContentFixture(overrides);
}

function createSavedPageContentFromInput(
  input: SaveFrontstageTabDocumentInput
): FrontstagePageContent {
  return createPageContent({
    schema: {
      rootUid: 'root-1',
      payload: input.payload
    },
    root: {
      uid: 'root-1',
      payload: input.payload
    }
  });
}

function createTestNodeId() {
  return crypto.randomUUID();
}

function FrontStagePageHarness({
  workspaceId = 'workspace-1',
  pageId,
  tabId,
  onNavigatePage,
  onNavigateTab,
  initialPageTree,
  pageContent,
  isPageContentLoading,
  hasPageContentLoadError,
  onRefreshPage
}: {
  workspaceId?: string;
  pageId?: string;
  tabId?: string;
  onNavigatePage?: (pageId?: string) => void;
  onNavigateTab?: (tab: FrontstagePageTab) => void;
  initialPageTree?: TestFrontStageTreeNode[];
  pageContent?: FrontstagePageContent;
  isPageContentLoading?: boolean;
  hasPageContentLoadError?: boolean;
  onRefreshPage?: () => Promise<void>;
}) {
  const [pageTree, setPageTree] = useState<TestFrontStageTreeNode[]>(
    initialPageTree ?? []
  );
  const fixtureBlocks =
    pageContent?.document.payload !== null &&
    typeof pageContent?.document.payload === 'object' &&
    !Array.isArray(pageContent.document.payload) &&
    Array.isArray(
      (pageContent.document.payload as Record<string, unknown>).blocks
    )
      ? ((pageContent.document.payload as Record<string, unknown>)
          .blocks as Array<Record<string, unknown>>)
      : [];
  const blockRoots = fixtureBlocks.map(
    (block, index): ConsoleFrontstageBlockNode => ({
      block_id: String(block.id),
      workspace_id: workspaceId,
      page_id: pageId ?? 'page-1',
      tab_id: tabId ?? pageContent?.tab.id ?? 'tab-1',
      parent_block_id: null,
      rank: String(index + 1).padStart(6, '0'),
      presentation: 'page',
      title: typeof block.title === 'string' ? block.title : null,
      description: null,
      schema_version: 1,
      code_ref:
        typeof block.codeRef === 'string'
          ? block.codeRef
          : `frontstage.block.${String(block.id)}`,
      input_mapping: {},
      output_mapping: {},
      runtime_descriptor: block,
      created_at: '2026-08-16T00:00:00Z',
      updated_at: '2026-08-16T00:00:00Z'
    })
  );

  return (
    <FrontStagePage
      workspaceId={workspaceId}
      pageId={pageId}
      tabId={tabId}
      onNavigatePage={onNavigatePage}
      onNavigateTab={onNavigateTab}
      initialPageTree={pageTree}
      pageContent={pageContent}
      blockRoots={blockRoots}
      isPageContentLoading={isPageContentLoading}
      hasPageContentLoadError={hasPageContentLoadError}
      onRefreshPage={onRefreshPage}
      onCreateGroupNode={(input) => {
        const groupNode = {
          id: createTestNodeId(),
          title: input.title,
          icon: input.icon,
          tooltip: input.tooltip,
          kind: 'group' as const,
          children: []
        };
        setPageTree((currentTree) => [...currentTree, groupNode]);
        return Promise.resolve({ id: groupNode.id, kind: groupNode.kind });
      }}
      onCreatePageNode={(input) => {
        const pageNode = {
          id: createTestNodeId(),
          title: input.title,
          icon: input.icon,
          tooltip: input.tooltip,
          kind: 'page' as const
        };
        setPageTree((currentTree) =>
          input.parentId
            ? insertPageIntoGroup(currentTree, input.parentId, pageNode)
            : [...currentTree, pageNode]
        );
        return Promise.resolve({ id: pageNode.id, kind: pageNode.kind });
      }}
      onRenamePageNode={(nodeId, input) => {
        setPageTree((currentTree) =>
          updateNodeMetadataInTree(
            renameNodeInTree(currentTree, nodeId, input.title ?? ''),
            nodeId,
            {
              icon: input.icon,
              tooltip: input.tooltip
            }
          )
        );
        return Promise.resolve({ id: nodeId, kind: 'page' });
      }}
      onMovePageNode={(nodeId, input) => {
        setPageTree((currentTree) =>
          moveNodeInTree(currentTree, nodeId, input.rank === '000000' ? -1 : 1)
        );
        return Promise.resolve({ id: nodeId, kind: 'page' });
      }}
      onDeletePageNode={(nodeId) => {
        setPageTree((currentTree) => removeNodeFromTree(currentTree, nodeId));
        return Promise.resolve();
      }}
    />
  );
}

function renderPage(
  pageId?: string,
  onNavigatePage?: (pageId?: string) => void
) {
  return render(
    <AppProviders>
      <FrontStagePageHarness
        pageId={pageId}
        onNavigatePage={onNavigatePage}
        initialPageTree={pageId ? [createBackendPage(pageId)] : undefined}
      />
    </AppProviders>
  );
}

async function hoverAddMenuAndFlush() {
  fireEvent.mouseEnter(screen.getByRole('button', { name: '添加菜单' }));
}

function activateDesignMode() {
  act(() => {
    useFrontstageDesignModeStore.getState().setDesignMode(true);
  });
}

function exitDesignMode() {
  act(() => {
    useFrontstageDesignModeStore.getState().setDesignMode(false);
  });
}

function mockPageContentSaveState(
  overrides: Partial<FrontstagePageContentSaveState> = {}
): FrontstagePageContentSaveState {
  const state = {
    save: vi.fn((input: SaveFrontstageTabDocumentInput) =>
      Promise.resolve(createSavedPageContentFromInput(input))
    ),
    saving: false,
    isPending: false,
    error: null,
    reset: vi.fn(),
    clearError: vi.fn(),
    ...overrides
  };

  pageContentSaveHook.useFrontstagePageContentSave.mockReturnValue(state);
  return state;
}

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: '1flowbase:frontstage.js-ui-block',
    runtimeKind: 'native_react',
    installationId: 'builtin-installation',
    providerCode: '1flowbase',
    pluginId: 'builtin-frontstage',
    pluginVersion: '1.0.0',
    contributionCode: 'frontstage.js-ui-block',
    title: '空白 JS Block',
    entry: 'index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: [],
      inputSchema: {}
    },
    uiCapabilities: [],
    codeCapabilities: {
      template: {
        source: PLUGIN_CODE_TEMPLATE,
        version: '2.4.0',
        language: 'tsx'
      }
    },
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

function mockFrontstageBlockCatalog(
  items: NormalizedFrontstageBlockCatalogEntry[] = [],
  overrides: { loading?: boolean; error?: Error | null } = {}
) {
  blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
    items,
    diagnostics: [],
    loading: overrides.loading ?? false,
    error: overrides.error ?? null
  });
}

beforeEach(() => {
  resetAuthStore();
  resetFrontstageDesignModeStore();
  vi.clearAllMocks();
  mockPageContentSaveState();
  mockFrontstageBlockCatalog();
  pageTabsApi.fetchFrontstagePageTabs.mockResolvedValue([
    {
      id: 'tab-1',
      page_id: 'page-1',
      title: '概览',
      rank: '001000',
      is_default: true,
      document_root_uid: 'frontstage.tab.tab-1.root'
    }
  ]);
  blockTreeApi.createFrontstageBlockNode.mockResolvedValue({
    block_id: 'created-root-block',
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id: null,
    rank: '001000',
    presentation: 'page',
    title: 'Created block',
    description: null,
    schema_version: 1,
    code_ref: 'frontstage.block.created-root-block',
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: {},
    created_at: '2026-08-16T00:00:00Z',
    updated_at: '2026-08-16T00:00:00Z'
  });
  blockTreeApi.updateFrontstageBlockDescriptors.mockResolvedValue([]);
  blockTreeApi.updateFrontstageBlockNode.mockImplementation(
    async (_workspaceId: string, _pageId: string, blockId: string) => ({
      block_id: blockId,
      workspace_id: 'workspace-1',
      page_id: 'page-1',
      tab_id: 'tab-1',
      parent_block_id: null
    })
  );
  blockTreeApi.fetchFrontstageBlockNode.mockResolvedValue({
    block_id: 'orders-block',
    workspace_id: 'workspace-1',
    page_id: 'page-1',
    tab_id: 'tab-1',
    parent_block_id: null,
    rank: '000001',
    presentation: 'page',
    title: 'Orders',
    description: null,
    schema_version: 1,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    input_mapping: {},
    output_mapping: {},
    runtime_descriptor: {}
  });
  blockTreeApi.fetchFrontstageBlockNodeCode.mockResolvedValue({
    block_id: 'orders-block',
    page_id: 'page-1',
    source_code: 'export default function Orders() { return null; }',
    source_sha256: 'source-sha',
    dependency_lock: []
  });
});

export {
  FrontStagePageHarness,
  SLOW_FRONTSTAGE_TEST_TIMEOUT,
  activateDesignMode,
  authenticate,
  blockTreeApi,
  createBackendPage,
  createCatalogEntry,
  createPageContent,
  exitDesignMode,
  hoverAddMenuAndFlush,
  mockFrontstageBlockCatalog,
  mockPageContentSaveState,
  pageContentSaveHook,
  renderPage
};
