import { apiFetch } from '../../transport';

import type {
  ConsoleFrontstageBlockDeleteImpact,
  ConsoleFrontstageBlockCodeFragment,
  ConsoleFrontstageBlockCodeFragmentQuery,
  ConsoleFrontstageBlockDescendant,
  ConsoleFrontstageBlockDescendantsQuery,
  ConsoleFrontstageBlockListQuery,
  ConsoleFrontstageBlockRootListQuery,
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
  PatchConsoleFrontstageBlockNodeCodeInput,
  SaveConsoleFrontstageBlockNodeCodeInput,
  UpdateConsoleFrontstageBlockDescriptorsInput,
  UpdateConsoleFrontstageBlockNodeInput
} from './types';

function blockTreePath(pageId: string): string {
  return `/api/console/frontstage/pages/${encodeURIComponent(pageId)}/blocks`;
}

function blockPath(pageId: string, blockId: string): string {
  return `${blockTreePath(pageId)}/${encodeURIComponent(blockId)}`;
}

function withQuery(path: string, values: object): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined) query.set(key, String(value));
  }
  return query.size > 0 ? `${path}?${query.toString()}` : path;
}

export function listConsoleFrontstageBlockRoots(
  pageId: string,
  query: ConsoleFrontstageBlockRootListQuery,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode[]> {
  return apiFetch({
    path: withQuery(blockTreePath(pageId), query),
    method: 'GET',
    baseUrl
  });
}

export function createConsoleFrontstageBlockNode(
  pageId: string,
  input: CreateConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockTreePath(pageId),
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function searchConsoleFrontstageBlocks(
  pageId: string,
  query: ConsoleFrontstageBlockSearchQuery,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockSearchResult[]> {
  return apiFetch({
    path: withQuery(`${blockTreePath(pageId)}/search`, query),
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockNode(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockPath(pageId, blockId),
    method: 'GET',
    baseUrl
  });
}

export function updateConsoleFrontstageBlockNode(
  pageId: string,
  blockId: string,
  input: UpdateConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: blockPath(pageId, blockId),
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleFrontstageBlockDescriptors(
  pageId: string,
  tabId: string,
  input: UpdateConsoleFrontstageBlockDescriptorsInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode[]> {
  return apiFetch({
    path: `/api/console/frontstage/pages/${encodeURIComponent(pageId)}/tabs/${encodeURIComponent(tabId)}/block-descriptors`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFrontstageBlockLeaf(
  pageId: string,
  blockId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch({
    path: blockPath(pageId, blockId),
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listConsoleFrontstageBlockChildren(
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockListQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeSummary[]> {
  return apiFetch({
    path: withQuery(
      `${blockPath(pageId, blockId)}/children`,
      query
    ),
    method: 'GET',
    baseUrl
  });
}

export function listConsoleFrontstageBlockAncestors(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeSummary[]> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/ancestors`,
    method: 'GET',
    baseUrl
  });
}

export function listConsoleFrontstageBlockDescendants(
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockDescendantsQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockDescendant[]> {
  return apiFetch({
    path: withQuery(
      `${blockPath(pageId, blockId)}/descendants`,
      query
    ),
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockDeleteImpact(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockDeleteImpact> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/delete-impact`,
    method: 'GET',
    baseUrl
  });
}

export function moveConsoleFrontstageBlockNode(
  pageId: string,
  blockId: string,
  input: MoveConsoleFrontstageBlockNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNode> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/move`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleFrontstageBlockSubtree(
  pageId: string,
  blockId: string,
  input: DeleteConsoleFrontstageBlockSubtreeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockSubtreeDeleteResult> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/delete-subtree`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleFrontstageBlockNodeCode(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeCode> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/code`,
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockCodeFragment(
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockCodeFragmentQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageBlockCodeFragment> {
  return apiFetch({
    path: withQuery(
      `${blockPath(pageId, blockId)}/code/fragment`,
      query
    ),
    method: 'GET',
    baseUrl
  });
}

export function getConsoleFrontstageBlockRuntimeAssembly(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockRuntimeAssembly> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/runtime-assembly`,
    method: 'GET',
    baseUrl
  });
}

export function openConsoleFrontstageBlock(
  pageId: string,
  blockId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockOpenTarget> {
  return apiFetch<ConsoleFrontstageBlockOpenTarget>({
    path: `${blockPath(pageId, blockId)}/open`,
    method: 'GET',
    ...(baseUrl ? { baseUrl } : {})
  });
}

export function saveConsoleFrontstageBlockNodeCode(
  pageId: string,
  blockId: string,
  input: SaveConsoleFrontstageBlockNodeCodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeCode> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/code`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function patchConsoleFrontstageBlockNodeCode(
  pageId: string,
  blockId: string,
  input: PatchConsoleFrontstageBlockNodeCodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockNodeCode> {
  return apiFetch({
    path: `${blockPath(pageId, blockId)}/code`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}
