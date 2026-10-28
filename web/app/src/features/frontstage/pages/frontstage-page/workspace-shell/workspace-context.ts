import { createContext, useContext } from 'react';
import type { usePageTreeWorkspace } from '../use-page-tree-workspace';
import { findNodeById, resolveSelectedPageId } from '../../../lib/page-tree';

type Workspace = ReturnType<typeof usePageTreeWorkspace>;
export const FrontstageWorkspaceContext = createContext<Workspace | null>(null);

export function useOptionalFrontstageWorkspace() {
  return useContext(FrontstageWorkspaceContext);
}

/** Page identity belongs to each retained body, never to the active shell route. */
export function useFrontstageWorkspace(pageId?: string) {
  const workspace = useOptionalFrontstageWorkspace();
  if (!workspace) throw new Error('Frontstage workspace shell is required');
  const selectedPageId = pageId
    ? resolveSelectedPageId({ pageId, pageTree: workspace.pageTree })
        .selectedPageId
    : workspace.selectedPageId;
  const selectedPageNode = selectedPageId
    ? findNodeById(workspace.pageTree, selectedPageId)
    : null;
  return {
    ...workspace,
    selectedPageId,
    selectedPageNode,
    handlePageTitleChange: (show_title: boolean) =>
      workspace.handlePageTitleChange(show_title, selectedPageNode),
    handlePageTabsEnabledChange: (enabled: boolean) =>
      workspace.handlePageTabsEnabledChange(enabled, selectedPageNode)
  };
}
