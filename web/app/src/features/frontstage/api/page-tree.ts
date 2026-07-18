import {
  createFrontstageGroup,
  createFrontstagePage,
  deleteFrontstagePageNode,
  getDefaultApiBaseUrl,
  listFrontstagePages,
  moveFrontstagePageNode,
  updateFrontstagePageNodeTitle,
  type ApiBaseUrlLocation,
  type ConsoleFrontstagePageNode,
  type ConsoleFrontstagePageTreeNode
} from '@1flowbase/api-client';

export type FrontstageNavigationPlacement = 'topbar' | 'sidebar';
export type FrontstagePageTreeNode = ConsoleFrontstagePageTreeNode & {
  placement: FrontstageNavigationPlacement;
  children: FrontstagePageTreeNode[];
};
export type FrontstagePageNode = ConsoleFrontstagePageNode;

export interface CreateFrontstageNodeInput {
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  parentId: string | null;
  rank: string;
  placement?: FrontstageNavigationPlacement;
  slug?: string | null;
}

export interface RenameFrontstageNodeInput {
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  contentPresentation?: 'single' | 'tabs';
  slug?: string | null;
}

export interface UpdateFrontstageNodeMetadataInput {
  title?: string | null;
  icon?: string | null;
  tooltip?: string | null;
  isHidden?: boolean;
}

export interface MoveFrontstageNodeInput {
  parentId: string | null;
  rank: string;
}

export const frontstagePageTreeQueryKey = (workspaceId: string) =>
  ['frontstage', workspaceId, 'page-tree'] as const;

export function getFrontstageApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

export function fetchFrontstagePageTree(
  workspaceId: string
): Promise<FrontstagePageTreeNode[]> {
  return listFrontstagePages(workspaceId, getFrontstageApiBaseUrl()) as Promise<
    FrontstagePageTreeNode[]
  >;
}

export function createFrontstagePageGroupNode(
  workspaceId: string,
  input: CreateFrontstageNodeInput,
  csrfToken: string
): Promise<FrontstagePageNode> {
  return createFrontstageGroup(
    workspaceId,
    {
      title: input.title,
      icon: input.icon,
      tooltip: input.tooltip,
      parent_id: input.parentId,
      rank: input.rank,
      placement: input.placement,
      slug: input.slug
    },
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function createFrontstagePageNode(
  workspaceId: string,
  input: CreateFrontstageNodeInput,
  csrfToken: string
): Promise<FrontstagePageNode> {
  return createFrontstagePage(
    workspaceId,
    {
      title: input.title,
      icon: input.icon,
      tooltip: input.tooltip,
      parent_id: input.parentId,
      rank: input.rank,
      placement: input.placement,
      slug: input.slug
    },
    csrfToken,
    getFrontstageApiBaseUrl()
  ).then((creation) => creation.page);
}

export function renameFrontstagePageNode(
  workspaceId: string,
  pageNodeId: string,
  input: RenameFrontstageNodeInput,
  csrfToken: string
): Promise<FrontstagePageNode> {
  const body: {
    title?: string | null;
    icon?: string | null;
    tooltip?: string | null;
    content_presentation?: 'single' | 'tabs';
    slug?: string | null;
  } = { title: input.title };
  if (Object.prototype.hasOwnProperty.call(input, 'icon')) {
    body.icon = input.icon;
  }
  if (Object.prototype.hasOwnProperty.call(input, 'tooltip')) {
    body.tooltip = input.tooltip;
  }
  if (Object.prototype.hasOwnProperty.call(input, 'contentPresentation')) {
    body.content_presentation = input.contentPresentation;
  }
  if (Object.prototype.hasOwnProperty.call(input, 'slug')) {
    body.slug = input.slug;
  }

  return updateFrontstagePageNodeTitle(
    workspaceId,
    pageNodeId,
    body,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function updateFrontstagePageNodeMetadata(
  workspaceId: string,
  pageNodeId: string,
  input: UpdateFrontstageNodeMetadataInput,
  csrfToken: string
): Promise<FrontstagePageNode> {
  return updateFrontstagePageNodeTitle(
    workspaceId,
    pageNodeId,
    {
      title: input.title,
      icon: input.icon,
      tooltip: input.tooltip,
      is_hidden: input.isHidden
    },
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function moveFrontstageNode(
  workspaceId: string,
  pageNodeId: string,
  input: MoveFrontstageNodeInput,
  csrfToken: string
): Promise<FrontstagePageNode> {
  return moveFrontstagePageNode(
    workspaceId,
    pageNodeId,
    {
      parent_id: input.parentId,
      rank: input.rank
    },
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function deleteFrontstageNode(
  workspaceId: string,
  pageNodeId: string,
  csrfToken: string
): Promise<void> {
  return deleteFrontstagePageNode(
    workspaceId,
    pageNodeId,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}
