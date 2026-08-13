import {
  getFrontstageBlockCode,
  saveFrontstageBlockCode as saveConsoleFrontstageBlockCode,
  type ConsoleFrontstageBlockCode,
  type SaveFrontstageBlockCodeInput as ConsoleSaveFrontstageBlockCodeInput
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageBlockCode = ConsoleFrontstageBlockCode;

export type SaveFrontstageBlockCodeInput =
  ConsoleSaveFrontstageBlockCodeInput & { code_ref: string };

export const frontstageBlockCodeQueryKey = (
  workspaceId: string,
  pageId: string,
  codeRef: string,
  actorId: string
) =>
  [
    'frontstage',
    actorId,
    workspaceId,
    'pages',
    pageId,
    'block-code',
    codeRef
  ] as const;

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

  return blockCode;
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
    input.code_ref,
    {
      source_code: input.source_code,
      dependency_lock: input.dependency_lock,
      tailwind_toolchain_lock: input.tailwind_toolchain_lock,
      generated_css: input.generated_css,
      generated_css_sha256: input.generated_css_sha256,
      compiler_identity: input.compiler_identity
    },
    csrfToken,
    getFrontstageApiBaseUrl()
  );

  return blockCode;
}
