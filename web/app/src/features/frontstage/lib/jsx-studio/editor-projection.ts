import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  contextComment: string;
  monacoExtraLibs: BlockSourceExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  catalogEntry,
  componentModuleSources = []
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  componentModuleSources?: readonly string[];
}): FrontstageJsxEditorProjection {
  const codeModules = catalogEntry?.codeModules ?? [];
  const monacoExtraLibs = [
    ...FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS,
    {
      source: 'tailwindcss',
      filePath: 'file:///node_modules/tailwindcss/index.d.ts',
      content: "declare module 'tailwindcss' {}\n"
    },
    ...codeModules.map((codeModule) => ({
      source: codeModule.source,
      filePath: `file:///node_modules/${codeModule.source}/index.d.ts`,
      content: codeModule.type_declarations
    }))
  ];
  return {
    allowedImportSources: new Set([
      'react',
      'react/jsx-runtime',
      'antd',
      'tailwindcss',
      ...componentModuleSources
    ]),
    contextComment: createFrontstageContextComment(),
    monacoExtraLibs
  };
}

export function mergeFrontstageComponentDeclarationExtraLibs(
  extraLibs: readonly BlockSourceExtraLib[],
  declarations: Readonly<Record<string, string>>
): BlockSourceExtraLib[] {
  const remainingDeclarations = new Map(
    Object.entries(declarations).filter(([, declaration]) => declaration)
  );
  const merged = extraLibs.map((extraLib) => {
    const declaration = remainingDeclarations.get(extraLib.source);
    if (!declaration) return extraLib;
    remainingDeclarations.delete(extraLib.source);
    return {
      ...extraLib,
      content: `${extraLib.content}\n\n${declaration}`
    };
  });
  for (const [source, content] of remainingDeclarations) {
    merged.push({
      source,
      filePath: `file:///node_modules/${source}/index.d.ts`,
      content
    });
  }
  return merged;
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
