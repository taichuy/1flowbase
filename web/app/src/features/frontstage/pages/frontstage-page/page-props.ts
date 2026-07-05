import type { FrontstagePageContent } from '../../api/page-content';
import type { FrontStageTreeNode } from '../../lib/page-tree';
import type {
  CreatePageTreeNodeInput,
  MovePageTreeNodeInput,
  PageTreeMutationResult,
  RenamePageTreeNodeInput,
  UpdatePageTreeNodeMetadataInput
} from './page-tree-operations';

export type FrontStagePageProps = {
  workspaceId: string;
  pageId?: string;
  onNavigatePage?: (pageId?: string) => void;
  initialPageTree?: FrontStageTreeNode[];
  isPageTreeLoading?: boolean;
  hasPageTreeLoadError?: boolean;
  onRetryLoadPageTree?: () => void;
  pageContent?: FrontstagePageContent;
  isPageContentLoading?: boolean;
  hasPageContentLoadError?: boolean;
  isPageContentPermissionDenied?: boolean;
  onRetryLoadPageContent?: () => void;
  isPageTreeMutating?: boolean;
  pageTreeMutationError?: Error | null;
  onCreateGroupNode?: (
    input: CreatePageTreeNodeInput
  ) => Promise<PageTreeMutationResult | void>;
  onCreatePageNode?: (
    input: CreatePageTreeNodeInput
  ) => Promise<PageTreeMutationResult | void>;
  onRenamePageNode?: (
    nodeId: string,
    input: RenamePageTreeNodeInput
  ) => Promise<PageTreeMutationResult | void>;
  onUpdatePageNodeMetadata?: (
    nodeId: string,
    input: UpdatePageTreeNodeMetadataInput
  ) => Promise<PageTreeMutationResult | void>;
  onMovePageNode?: (
    nodeId: string,
    input: MovePageTreeNodeInput
  ) => Promise<PageTreeMutationResult | void>;
  onDeletePageNode?: (nodeId: string) => Promise<void>;
};
