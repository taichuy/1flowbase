import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';

export interface FrontstageJsxEditorProjection {
  componentCatalogQuery: {
    installation_id: string;
    contribution_code: string;
  } | null;
  contextComment: string;
  monacoExtraLibs: FrontendBlockMonacoExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  catalogEntry
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
}): FrontstageJsxEditorProjection {
  const monacoExtraLibs = catalogEntry?.codeCapabilities?.monacoExtraLibs ?? [];
  return {
    componentCatalogQuery: catalogEntry
      ? {
          installation_id: catalogEntry.installationId,
          contribution_code: catalogEntry.contributionCode
        }
      : null,
    contextComment: createFrontstageContextComment(),
    monacoExtraLibs
  };
}

export function createFrontstageContextComment(): string {
  return [
    '/**',
    ' * @1flowbase-context',
    ' * inputs: 无',
    ' * outputs: 无',
    ' */'
  ].join('\n');
}
