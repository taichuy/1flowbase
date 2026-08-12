import { apiFetch } from '../../transport';

import type {
  ConsoleFrontstageBlockDeleteImpact,
  ConsoleFrontstageBlockDescendant,
  ConsoleFrontstageBlockDescendantsQuery,
  ConsoleFrontstageBlockListQuery,
  ConsoleFrontstageBlockNode,
  ConsoleFrontstageBlockNodeCode,
  ConsoleFrontstageBlockRuntimeAssembly,
  ConsoleFrontstageBlockOpenTarget,
  ConsoleFrontstageBlockNodeSummary,
  ConsoleFrontstageBlockSearchQuery,
  ConsoleFrontstageBlockSearchResult,
  ConsoleFrontstageBlockSubtreeDeleteResult,
  CreateConsoleFrontstageBlockNodeInput,
  DeleteConsoleFrontstageBlockSubtreeInput,
  MoveConsoleFrontstageBlockNodeInput,
  SaveConsoleFrontstageBlockNodeCodeInput,
  UpdateConsoleFrontstageBlockNodeInput
} from './types';

function blockTreePath(workspaceId: string, pageId: string): string {
  return `/api/console/frontstage/${encodeURIComponent(workspaceId)}/pages/${encodeURIComponent(pageId)}/blocks`;
}

function blockPath(
  workspaceId: string,
  pageId: string,
  blockId: string
): string {
  return `${blockTreePath(workspaceId, pageId)}/${encodeURIComponent(blockId)}`;
}

function withQuery(path: string, values: object): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined) query.set(key, String(value));
  }
  return query.size > 0 ? `${path}?${query.toString()}` : path;
}

export function listConsoleFrontstageBlockRoots(
  workspaceId: string,
  pageId: string,
  query: ConsoleFrontstageBlockListQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeSummary[]> {
  return apiFetch({
    path: withQuery(blockTreePath(workspaceId, pageId), query),
    method: 'GET',
    baseUrl
  });
}

export function createConsoleFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  input: CreateConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockTreePath(workspaceId, pageId),
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function searchConsoleFrontstageBlocks(
  workspaceId: string,
  pageId: string,
  query: ConsoleFrontstageBlockSearchQuery,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockSearchResult[]> {
  return apiFetch({
    path: withQuery(`${blockTreePath(workspaceId, pageId)}/search`, query),
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockPath(workspaceId, pageId, blockId),
    method: 'GET',
    baseUrl
  });
}

export function updateConsoleFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: UpdateConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockPath(workspaceId, pageId, blockId),
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFrontstageBlockLeaf(
  workspaceId: string,
  pageId: string,
  blockId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch({
    path: blockPath(workspaceId, pageId, blockId),
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listConsoleFrontstageBlockChildren(
  workspaceId: string,
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockListQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeSummary[]> {
  return apiFetch({
    path: withQuery(
      `${blockPath(workspaceId, pageId, blockId)}/children`,
      query
    ),
    method: 'GET',
    baseUrl
  });
}

export function listConsoleFrontstageBlockAncestors(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeSummary[]> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/ancestors`,
    method: 'GET',
    baseUrl
  });
}

export function listConsoleFrontstageBlockDescendants(
  workspaceId: string,
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockDescendantsQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockDescendant[]> {
  return apiFetch({
    path: withQuery(
      `${blockPath(workspaceId, pageId, blockId)}/descendants`,
      query
    ),
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockDeleteImpact(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockDeleteImpact> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/delete-impact`,
    method: 'GET',
    baseUrl
  });
}

export function moveConsoleFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: MoveConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/move`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFrontstageBlockSubtree(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: DeleteConsoleFrontstageBlockSubtreeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockSubtreeDeleteResult> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/delete-subtree`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleFrontstageBlockNodeCode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeCode> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/code`,
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockRuntimeAssembly(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockRuntimeAssembly> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/runtime-assembly`,
    method: 'GET',
    baseUrl
  });
}

export function openConsoleFrontstageBlock(
  workspaceId: string,
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockOpenTarget> {
  return apiFetch<ConsoleFrontstageBlockOpenTarget>({
    path: `${blockPath(workspaceId, pageId, blockId)}/open`,
    method: 'GET',
    ...(baseUrl ? { baseUrl } : {})
  });
}

export function saveConsoleFrontstageBlockNodeCode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: SaveConsoleFrontstageBlockNodeCodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeCode> {
  return apiFetch({
    path: `${blockPath(workspaceId, pageId, blockId)}/code`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
