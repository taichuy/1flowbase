import type {
  FrontendBlockCodeModuleSource,
  FrontendBlockMonacoExtraLib
} from '@1flowbase/page-protocol';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';

export interface FrontstageJsxComponent {
  name: string;
  moduleSource: FrontendBlockCodeModuleSource;
}

export interface FrontstageJsxEditorProjection {
  components: FrontstageJsxComponent[];
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
): FrontstageJsxComponent[] {
  const components = new Map<string, FrontstageJsxComponent>();
  const pattern = /export\s+(?:declare\s+)?const\s+([A-Z][A-Za-z0-9_$]*)\b/g;
  for (const extraLib of extraLibs) {
    for (const match of extraLib.content.matchAll(pattern)) {
      if (match[1]) {
        const component = {
          name: match[1],
          moduleSource: extraLib.source
        };
        components.set(
          `${component.moduleSource}:${component.name}`,
          component
        );
      }
    }
  }
  return [...components.values()].sort((left, right) =>
    left.name.localeCompare(right.name)
  );
}
