import {
  createConsoleFrontstageBlockNode,
  deleteConsoleFrontstageBlockLeaf,
  deleteConsoleFrontstageBlockSubtree,
  getConsoleFrontstageBlockDeleteImpact,
  getConsoleFrontstageBlockNode,
  getConsoleFrontstageBlockNodeCode,
  getConsoleFrontstageBlockRuntimeAssembly,
  listConsoleFrontstageBlockAncestors,
  listConsoleFrontstageBlockChildren,
  listConsoleFrontstageBlockDescendants,
  listConsoleFrontstageBlockRoots,
  moveConsoleFrontstageBlockNode,
  openConsoleFrontstageBlock,
  saveConsoleFrontstageBlockNodeCode,
  searchConsoleFrontstageBlocks,
  updateConsoleFrontstageBlockNode,
  type ConsoleFrontstageBlockDescendantsQuery,
  type ConsoleFrontstageBlockListQuery,
  type ConsoleFrontstageBlockSearchQuery,
  type CreateConsoleFrontstageBlockNodeInput,
  type DeleteConsoleFrontstageBlockSubtreeInput,
  type MoveConsoleFrontstageBlockNodeInput,
  type SaveConsoleFrontstageBlockNodeCodeInput,
  type UpdateConsoleFrontstageBlockNodeInput
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

const pageKey = (workspaceId: string, pageId: string) =>
  ['frontstage', workspaceId, 'pages', pageId, 'block-tree'] as const;

export const frontstageBlockTreeQueryKeys = {
  page: pageKey,
  roots: (
    workspaceId: string,
    pageId: string,
    query: ConsoleFrontstageBlockListQuery = {}
  ) => [...pageKey(workspaceId, pageId), 'roots', query] as const,
  children: (
    workspaceId: string,
    pageId: string,
    blockId: string,
    query: ConsoleFrontstageBlockListQuery = {}
  ) =>
    [
      ...pageKey(workspaceId, pageId),
      'blocks',
      blockId,
      'children',
      query
    ] as const,
  block: (workspaceId: string, pageId: string, blockId: string) =>
    [...pageKey(workspaceId, pageId), 'blocks', blockId, 'detail'] as const,
  ancestors: (workspaceId: string, pageId: string, blockId: string) =>
    [...pageKey(workspaceId, pageId), 'blocks', blockId, 'ancestors'] as const,
  descendants: (
    workspaceId: string,
    pageId: string,
    blockId: string,
    query: ConsoleFrontstageBlockDescendantsQuery = {}
  ) =>
    [
      ...pageKey(workspaceId, pageId),
      'blocks',
      blockId,
      'descendants',
      query
    ] as const,
  deleteImpact: (workspaceId: string, pageId: string, blockId: string) =>
    [
      ...pageKey(workspaceId, pageId),
      'blocks',
      blockId,
      'delete-impact'
    ] as const,
  code: (workspaceId: string, pageId: string, blockId: string) =>
    [...pageKey(workspaceId, pageId), 'blocks', blockId, 'code'] as const,
  runtimeAssembly: (workspaceId: string, pageId: string, blockId: string) =>
    [
      ...pageKey(workspaceId, pageId),
      'blocks',
      blockId,
      'runtime-assembly'
    ] as const,
  searches: (workspaceId: string, pageId: string) =>
    [...pageKey(workspaceId, pageId), 'search'] as const,
  search: (
    workspaceId: string,
    pageId: string,
    query: ConsoleFrontstageBlockSearchQuery
  ) => [...pageKey(workspaceId, pageId), 'search', query] as const
};

export function fetchFrontstageBlockRoots(
  workspaceId: string,
  pageId: string,
  query: ConsoleFrontstageBlockListQuery = {}
) {
  return listConsoleFrontstageBlockRoots(
    workspaceId,
    pageId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function createFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  input: CreateConsoleFrontstageBlockNodeInput,
  csrfToken: string
) {
  return createConsoleFrontstageBlockNode(
    workspaceId,
    pageId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function searchFrontstageBlocks(
  workspaceId: string,
  pageId: string,
  query: ConsoleFrontstageBlockSearchQuery
) {
  return searchConsoleFrontstageBlocks(
    workspaceId,
    pageId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return getConsoleFrontstageBlockNode(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function openFrontstageBlock(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return openConsoleFrontstageBlock(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function updateFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: UpdateConsoleFrontstageBlockNodeInput,
  csrfToken: string
) {
  return updateConsoleFrontstageBlockNode(
    workspaceId,
    pageId,
    blockId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function deleteFrontstageBlockLeaf(
  workspaceId: string,
  pageId: string,
  blockId: string,
  csrfToken: string
) {
  return deleteConsoleFrontstageBlockLeaf(
    workspaceId,
    pageId,
    blockId,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockChildren(
  workspaceId: string,
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockListQuery = {}
) {
  return listConsoleFrontstageBlockChildren(
    workspaceId,
    pageId,
    blockId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockAncestors(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return listConsoleFrontstageBlockAncestors(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockDescendants(
  workspaceId: string,
  pageId: string,
  blockId: string,
  query: ConsoleFrontstageBlockDescendantsQuery = {}
) {
  return listConsoleFrontstageBlockDescendants(
    workspaceId,
    pageId,
    blockId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockDeleteImpact(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return getConsoleFrontstageBlockDeleteImpact(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function moveFrontstageBlockNode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: MoveConsoleFrontstageBlockNodeInput,
  csrfToken: string
) {
  return moveConsoleFrontstageBlockNode(
    workspaceId,
    pageId,
    blockId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function deleteFrontstageBlockSubtree(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: DeleteConsoleFrontstageBlockSubtreeInput,
  csrfToken: string
) {
  return deleteConsoleFrontstageBlockSubtree(
    workspaceId,
    pageId,
    blockId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockNodeCode(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return getConsoleFrontstageBlockNodeCode(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageBlockRuntimeAssembly(
  workspaceId: string,
  pageId: string,
  blockId: string
) {
  return getConsoleFrontstageBlockRuntimeAssembly(
    workspaceId,
    pageId,
    blockId,
    getFrontstageApiBaseUrl()
  );
}

export function saveFrontstageBlockNodeCode(
  workspaceId: string,
  pageId: string,
  blockId: string,
  input: SaveConsoleFrontstageBlockNodeCodeInput,
  csrfToken: string
) {
  return saveConsoleFrontstageBlockNodeCode(
    workspaceId,
    pageId,
    blockId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export type {
  ConsoleFrontstageBlockDeleteImpact as FrontstageBlockDeleteImpact,
  ConsoleFrontstageBlockDescendant as FrontstageBlockDescendant,
  ConsoleFrontstageBlockDescendantsQuery as FrontstageBlockDescendantsQuery,
  ConsoleFrontstageBlockListQuery as FrontstageBlockListQuery,
  ConsoleFrontstageBlockNode as FrontstageBlockNode,
  ConsoleFrontstageBlockNodeCode as FrontstageBlockNodeCode,
  ConsoleFrontstageBlockRuntimeAssembly as FrontstageBlockRuntimeAssembly,
  ConsoleFrontstageBlockRuntimeLayer as FrontstageBlockRuntimeLayer,
  ConsoleFrontstageBlockNodeSummary as FrontstageBlockNodeSummary,
  ConsoleFrontstageBlockPresentation as FrontstageBlockPresentation,
  ConsoleFrontstageBlockSearchQuery as FrontstageBlockSearchQuery,
  ConsoleFrontstageBlockSearchResult as FrontstageBlockSearchResult,
  ConsoleFrontstageBlockSubtreeDeleteResult as FrontstageBlockSubtreeDeleteResult,
  CreateConsoleFrontstageBlockNodeInput as CreateFrontstageBlockNodeInput,
  DeleteConsoleFrontstageBlockSubtreeInput as DeleteFrontstageBlockSubtreeInput,
  MoveConsoleFrontstageBlockNodeInput as MoveFrontstageBlockNodeInput,
  SaveConsoleFrontstageBlockNodeCodeInput as SaveFrontstageBlockNodeCodeInput,
  UpdateConsoleFrontstageBlockNodeInput as UpdateFrontstageBlockNodeInput
} from '@1flowbase/api-client';
