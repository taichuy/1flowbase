import { Typography } from 'antd';
import { createContext, useContext, useState, type ReactNode } from 'react';

import { i18nText } from '../../../../../shared/i18n/text';
import { SectionPageLayout } from '../../../../../shared/ui/section-page-layout/SectionPageLayout';
import { useAuthStore } from '../../../../../state/auth-store';
import { useFrontstageDesignModeStore } from '../../../../../state/frontstage-design-mode-store';
import { MovePageModal } from '../../../components/page-tree-actions/MovePageModal';
import { FrontStagePageTreeSidebar } from '../../../components/FrontStagePageTreeSidebar';
import { findNodeById, resolveSelectedPageId } from '../../../lib/page-tree';
import { DESIGN_MODE_PERMISSION } from '../page-constants';
import type { FrontStagePageProps } from '../page-props';
import { PageTreeFormModal } from '../page-tree-form-modal';
import { usePageTreeWorkspace } from '../use-page-tree-workspace';
import './workspace-shell.css';

type Workspace = ReturnType<typeof usePageTreeWorkspace>;
const FrontstageWorkspaceContext = createContext<Workspace | null>(null);

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
    handlePageTabsEnabledChange: (enabled: boolean) =>
      workspace.handlePageTabsEnabledChange(enabled, selectedPageNode)
  };
}

/** One tree, form and navigation shell outside all retained page bodies. */
export function FrontstageWorkspaceShell({
  children,
  showSidebar = true,
  autoSelectFirstPage = true,
  ...props
}: FrontStagePageProps & { children: ReactNode }) {
  const workspace = usePageTreeWorkspace({ ...props, autoSelectFirstPage });
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const isDesignMode = useFrontstageDesignModeStore(
    (state) => state.isDesignMode
  );
  const canEdit =
    isDesignMode &&
    (actor?.effective_display_role === 'root' ||
      Boolean(me?.permissions.includes(DESIGN_MODE_PERMISSION)));
  const [movePageId, setMovePageId] = useState<string | null>(null);
  const movePage = movePageId
    ? findNodeById(workspace.pageTree, movePageId)
    : null;
  const sidebar =
    props.initialPageTree === undefined &&
    (props.isPageTreeLoading || props.hasPageTreeLoadError) ? (
      <Typography.Text type="secondary" style={{ paddingInline: 16 }}>
        {i18nText(
          'frontstage',
          props.isPageTreeLoading
            ? 'auto.page_tree_loading'
            : 'auto.page_tree_unavailable'
        )}
      </Typography.Text>
    ) : (
      <FrontStagePageTreeSidebar
        pageTree={workspace.pageTree}
        selectedPageId={workspace.selectedPageId}
        canEdit={canEdit}
        isOperationPending={workspace.isOperationPending}
        onAddGroup={workspace.handleAddGroup}
        onAddPage={workspace.handleAddPage}
        onAddPageInGroup={workspace.handleAddPageInGroup}
        onRenameNode={workspace.handleRenameNode}
        onUpdateNodeMetadata={workspace.handleUpdateNodeMetadata}
        onEditNodeTooltip={workspace.handleEditNodeTooltip}
        onAddNodeAtPosition={workspace.handleAddNodeAtPosition}
        onMoveNodeToPosition={workspace.handleMoveNodeToPosition}
        onOpenMovePage={setMovePageId}
        onDeleteNode={workspace.handleDeleteNode}
        onSelectPage={workspace.handleSelectPage}
      />
    );
  return (
    <FrontstageWorkspaceContext.Provider value={workspace}>
      <div
        className={
          showSidebar
            ? 'frontstage-workspace-shell'
            : 'frontstage-workspace-shell frontstage-workspace-shell--without-sidebar'
        }
      >
        <SectionPageLayout
          pageTitle={
            props.initialPageTree === undefined &&
            (props.isPageTreeLoading || props.hasPageTreeLoadError)
              ? i18nText('frontstage', 'auto.frontstage')
              : undefined
          }
          navItems={[]}
          activeKey=""
          contentWidth="wide"
          heightMode="viewport"
          sidebarContent={showSidebar ? sidebar : <></>}
        >
          {children}
        </SectionPageLayout>
      </div>
      {canEdit && movePage ? (
        <MovePageModal
          key={movePage.id}
          node={movePage}
          pageTree={props.navigationPageTree ?? workspace.pageTree}
          isOperationPending={workspace.isOperationPending}
          onMove={workspace.handleMovePageToGroup}
          onCancel={() => setMovePageId(null)}
        />
      ) : null}
      <PageTreeFormModal
        dialog={workspace.pageTreeFormDialog}
        form={workspace.pageTreeForm}
        iconPickerOpen={workspace.isPageTreeIconPickerOpen}
        isOperationPending={workspace.isOperationPending}
        onCancel={() => workspace.setPageTreeFormDialog(null)}
        onIconPickerOpenChange={workspace.setIsPageTreeIconPickerOpen}
        onSubmit={() => void workspace.handleSubmitPageTreeForm()}
      />
    </FrontstageWorkspaceContext.Provider>
  );
}
