import {
  getFrontstageBlockCode,
  saveFrontstageBlockCode as saveConsoleFrontstageBlockCode,
  type ConsoleFrontstageBlockCode
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export interface FrontstageBlockCode {
  pageId: string;
  codeRef: string;
  code: string;
  source_sha256: string;
}

export interface SaveFrontstageBlockCodeInput {
  codeRef: string;
  code: string;
}

export const frontstageBlockCodeQueryKey = (
  workspaceId: string,
  pageId: string,
  codeRef: string
) =>
  ['frontstage', workspaceId, 'pages', pageId, 'block-code', codeRef] as const;

function mapFrontstageBlockCode(
  blockCode: ConsoleFrontstageBlockCode
): FrontstageBlockCode {
  return {
    pageId: blockCode.page_id,
    codeRef: blockCode.code_ref,
    code: blockCode.code,
    source_sha256: blockCode.source_sha256
  };
}

export async function fetchFrontstageBlockCode(
  workspaceId: string,
  pageId: string,
  codeRef: string
): Promise<FrontstageBlockCode> {
  const blockCode = await getFrontstageBlockCode(
    workspaceId,
    pageId,
    codeRef,
    getFrontstageApiBaseUrl()
  );

  return mapFrontstageBlockCode(blockCode);
}

export async function saveFrontstageBlockCode(
  workspaceId: string,
  pageId: string,
  input: SaveFrontstageBlockCodeInput,
  csrfToken: string
): Promise<FrontstageBlockCode> {
  const blockCode = await saveConsoleFrontstageBlockCode(
    workspaceId,
    pageId,
    input.codeRef,
    { code: input.code },
    csrfToken,
    getFrontstageApiBaseUrl()
  );

  return mapFrontstageBlockCode(blockCode);
}
