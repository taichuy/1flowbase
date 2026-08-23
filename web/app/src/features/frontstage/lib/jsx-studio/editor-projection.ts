import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  contextComment: string;
  monacoExtraLibs: BlockSourceExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  catalogEntry
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
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
      ...codeModules.map((codeModule) => codeModule.source)
    ]),
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
