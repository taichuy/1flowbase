import { useQuery } from '@tanstack/react-query';
import { Navigate, useNavigate } from '@tanstack/react-router';
import { Result } from 'antd';
import { Suspense } from 'react';

import {
  FRONTSTAGE_SLUG_PAGE_BLOCK_PATH,
  FRONTSTAGE_SLUG_PAGE_PATH,
  FRONTSTAGE_SLUG_PAGE_TAB_PATH,
  FRONTSTAGE_SLUG_PATH
} from '../../../routes/route-config';
import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import {
  fetchFrontstagePageContent,
  frontstagePageContentQueryKey
} from '../api/page-content';
import {
  fetchFrontstagePageTabs,
  frontstagePageTabsQueryKey,
  type FrontstagePageTab
} from '../api/page-tabs';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type FrontstagePageTreeNode
} from '../api/page-tree';
import { useFrontstagePageTreeMutations } from '../hooks/use-frontstage-page-tree-mutations';
import {
  isForbiddenResponseError,
  isNotFoundResponseError
} from '../lib/api-errors';
import {
  fetchFrontstageBlockRuntimeAssembly,
  frontstageBlockTreeQueryKeys
} from '../api/block-tree';
import {
  getFirstTopLevelPageId,
  resolveSelectedPageId
} from '../lib/page-tree';
import { FrontStagePage } from './FrontStagePage';

export interface FrontstageWorkspacePageProps {
  workspaceId: string;
  pageId?: string;
  tabRef?: string;
  blockId?: string;
  rootNode?: FrontstagePageTreeNode;
}

export function FrontstageWorkspacePage({
  workspaceId,
  pageId,
  tabRef,
  blockId,
  rootNode
}: FrontstageWorkspacePageProps) {
  const navigate = useNavigate();
  const pageTreeQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId),
    queryFn: () => fetchFrontstagePageTree(workspaceId),
    retry: false
  });
  const pageTreeMutations = useFrontstagePageTreeMutations(workspaceId);
  const pageTreeFromApi =
    rootNode?.kind === 'group'
      ? rootNode.children
      : rootNode?.kind === 'page'
        ? [rootNode]
        : pageTreeQuery.data;
  const scopedPageTreeRootId = rootNode?.kind === 'group' ? rootNode.id : null;
  const resolvePageTreeParentId = (parentId: string | null) =>
    parentId ?? scopedPageTreeRootId;
  const effectivePageId = rootNode?.kind === 'page' ? rootNode.id : pageId;
  const selectedPageId =
    rootNode?.kind === 'page'
      ? rootNode.id
      : pageTreeFromApi
        ? rootNode?.kind === 'group' && !pageId
          ? getFirstTopLevelPageId(pageTreeFromApi)
          : resolveSelectedPageId({
              pageId: effectivePageId,
              pageTree: pageTreeFromApi
            }).selectedPageId
        : null;
  const blockRuntimeAssemblyQuery = useQuery({
    queryKey:
      selectedPageId && blockId
        ? frontstageBlockTreeQueryKeys.runtimeAssembly(
            workspaceId,
            selectedPageId,
            blockId
          )
        : [
            'frontstage',
            workspaceId,
            'pages',
            'unselected',
            'blocks',
            'runtime-assembly'
          ],
    queryFn: () => {
      if (!selectedPageId || !blockId) {
        throw new Error(
          'Frontstage runtime assembly query requires page and block ids'
        );
      }
      return fetchFrontstageBlockRuntimeAssembly(
        workspaceId,
        selectedPageId,
        blockId
      );
    },
    enabled: Boolean(selectedPageId && blockId),
    retry: false
  });
  const pageTabsQuery = useQuery({
    queryKey: selectedPageId
      ? frontstagePageTabsQueryKey(workspaceId, selectedPageId)
      : ['frontstage', workspaceId, 'pages', 'unselected', 'tabs'],
    queryFn: () => {
      if (!selectedPageId) {
        throw new Error('FrontStage page tabs query requires selected page');
      }

      return fetchFrontstagePageTabs(workspaceId, selectedPageId);
    },
    enabled: Boolean(selectedPageId && !blockId),
    retry: false
  });
  const defaultTabs = pageTabsQuery.data?.filter((tab) => tab.is_default) ?? [];
  const defaultTab = defaultTabs.length === 1 ? defaultTabs[0] : undefined;
  const runtimeTarget = blockRuntimeAssemblyQuery.data?.layers.at(-1);
  const tabReference = runtimeTarget?.tab_id ?? tabRef ?? defaultTab?.id;
  const shouldLoadPageContent = Boolean(
    effectivePageId && selectedPageId && tabReference && !blockId
  );
  const pageContentQuery = useQuery({
    queryKey:
      selectedPageId && tabReference
        ? frontstagePageContentQueryKey(
            workspaceId,
            selectedPageId,
            tabReference
          )
        : ['frontstage', workspaceId, 'pages', 'unselected', 'content'],
    queryFn: () => {
      if (!selectedPageId) {
        throw new Error('FrontStage page content query requires selected page');
      }

      if (!tabReference) {
        throw new Error('FrontStage page content query requires selected tab');
      }

      return fetchFrontstagePageContent(
        workspaceId,
        selectedPageId,
        tabReference
      );
    },
    enabled: shouldLoadPageContent,
    retry: false
  });

  if (rootNode?.kind === 'page' && !pageId && rootNode.slug && !blockId) {
    return (
      <Navigate
        to={FRONTSTAGE_SLUG_PAGE_PATH}
        params={{ slug: rootNode.slug, pageId: rootNode.id }}
        replace
      />
    );
  }

  if (
    rootNode?.kind === 'group' &&
    !pageId &&
    selectedPageId &&
    rootNode.slug &&
    !blockId
  ) {
    return (
      <Navigate
        to={FRONTSTAGE_SLUG_PAGE_PATH}
        params={{ slug: rootNode.slug, pageId: selectedPageId }}
        replace
      />
    );
  }

  if (
    selectedPageId &&
    !tabRef &&
    !blockId &&
    pageTabsQuery.data &&
    defaultTabs.length !== 1
  ) {
    return (
      <Result
        status="error"
        title={i18nText('frontstage', 'auto.page_tabs_invalid_default_title')}
        subTitle={i18nText(
          'frontstage',
          'auto.page_tabs_invalid_default_detail'
        )}
      />
    );
  }

  const resolvedTab = pageContentQuery.data?.tab;
  if (selectedPageId && tabRef && resolvedTab && rootNode?.slug && !blockId) {
    if (resolvedTab.isDefault) {
      return (
        <Navigate
          to={FRONTSTAGE_SLUG_PAGE_PATH}
          params={{ slug: rootNode.slug, pageId: selectedPageId }}
          replace
        />
      );
    }

    if (resolvedTab.routeSegment && tabRef !== resolvedTab.routeSegment) {
      return (
        <Navigate
          to={FRONTSTAGE_SLUG_PAGE_TAB_PATH}
          params={{
            slug: rootNode.slug,
            pageId: selectedPageId,
            tabRef: resolvedTab.routeSegment
          }}
          replace
        />
      );
    }
  }

  if (blockId && isNotFoundResponseError(blockRuntimeAssemblyQuery.error)) {
    return (
      <Result status="404" title={i18nText('app', 'auto.page_not_found')} />
    );
  }

  return (
    <Suspense fallback={<LoadingState fullscreen />}>
      <FrontStagePage
        workspaceId={workspaceId}
        pageId={effectivePageId}
        tabId={resolvedTab?.id ?? runtimeTarget?.tab_id}
        blockRuntimeAssembly={blockRuntimeAssemblyQuery.data}
        isBlockRuntimeRoute={Boolean(blockId)}
        isBlockRuntimeLoading={Boolean(
          blockId && blockRuntimeAssemblyQuery.isLoading
        )}
        hasBlockRuntimeLoadError={Boolean(
          blockId && blockRuntimeAssemblyQuery.isError
        )}
        isBlockRuntimePermissionDenied={Boolean(
          blockId && isForbiddenResponseError(blockRuntimeAssemblyQuery.error)
        )}
        onRetryLoadBlockRuntime={() => {
          void blockRuntimeAssemblyQuery.refetch();
        }}
        onNavigateBlock={(nextBlockId, replace = false) => {
          if (!rootNode?.slug || !selectedPageId) return;
          void navigate(
            nextBlockId
              ? {
                  to: FRONTSTAGE_SLUG_PAGE_BLOCK_PATH,
                  params: {
                    slug: rootNode.slug,
                    pageId: selectedPageId,
                    blockId: nextBlockId
                  },
                  replace
                }
              : {
                  to: FRONTSTAGE_SLUG_PAGE_PATH,
                  params: { slug: rootNode.slug, pageId: selectedPageId },
                  replace
                }
          );
        }}
        showSidebar={rootNode?.kind !== 'page'}
        autoSelectFirstPage={rootNode?.kind !== 'group' || Boolean(pageId)}
        initialPageTree={pageTreeFromApi}
        isPageTreeLoading={pageTreeQuery.isLoading}
        hasPageTreeLoadError={pageTreeQuery.isError}
        pageContent={pageContentQuery.data}
        isPageContentLoading={pageContentQuery.isLoading}
        hasPageContentLoadError={pageContentQuery.isError}
        isPageContentPermissionDenied={isForbiddenResponseError(
          pageContentQuery.error
        )}
        isPageTreeMutating={pageTreeMutations.isPending}
        pageTreeMutationError={pageTreeMutations.error}
        onCreateGroupNode={(input) =>
          pageTreeMutations.createGroup({
            ...input,
            parentId: resolvePageTreeParentId(input.parentId)
          })
        }
        onCreatePageNode={(input) =>
          pageTreeMutations.createPage({
            ...input,
            parentId: resolvePageTreeParentId(input.parentId)
          })
        }
        onRenamePageNode={pageTreeMutations.renameNode}
        onUpdatePageNodeMetadata={pageTreeMutations.updateNodeMetadata}
        onMovePageNode={(pageNodeId, input) =>
          pageTreeMutations.moveNode(pageNodeId, {
            ...input,
            parentId: resolvePageTreeParentId(input.parentId)
          })
        }
        onDeletePageNode={pageTreeMutations.deleteNode}
        onRetryLoadPageTree={() => {
          void pageTreeQuery.refetch();
        }}
        onRetryLoadPageContent={() => {
          void pageContentQuery.refetch();
        }}
        onNavigatePage={(nextPageId) => {
          if (!rootNode?.slug) return;
          void navigate(
            nextPageId
              ? {
                  to: FRONTSTAGE_SLUG_PAGE_PATH,
                  params: { slug: rootNode.slug, pageId: nextPageId }
                }
              : {
                  to: FRONTSTAGE_SLUG_PATH,
                  params: { slug: rootNode.slug }
                }
          );
        }}
        onNavigateTab={(nextTab: FrontstagePageTab) => {
          if (!selectedPageId) return;
          if (!rootNode?.slug) return;
          if (nextTab.is_default) {
            void navigate({
              to: FRONTSTAGE_SLUG_PAGE_PATH,
              params: { slug: rootNode.slug, pageId: selectedPageId }
            });
            return;
          }
          if (!nextTab.route_segment) return;
          void navigate({
            to: FRONTSTAGE_SLUG_PAGE_TAB_PATH,
            params: {
              slug: rootNode.slug,
              pageId: selectedPageId,
              tabRef: nextTab.route_segment
            }
          });
        }}
      />
    </Suspense>
  );
}
