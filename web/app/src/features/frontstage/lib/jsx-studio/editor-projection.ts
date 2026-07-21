import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';

export interface FrontstageJsxEditorProjection {
  components: string[];
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
    components: collectCatalogComponents(monacoExtraLibs),
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

function collectCatalogComponents(
  extraLibs: readonly FrontendBlockMonacoExtraLib[]
): string[] {
  const names = new Set<string>();
  const pattern = /export\s+(?:declare\s+)?const\s+([A-Z][A-Za-z0-9_$]*)\b/g;
  for (const extraLib of extraLibs) {
    for (const match of extraLib.content.matchAll(pattern)) {
      if (match[1]) names.add(match[1]);
    }
  }
  return [...names].sort((left, right) => left.localeCompare(right));
}
