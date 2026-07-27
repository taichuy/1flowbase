import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  componentCatalogQuery: {
    installation_id: string;
    contribution_code: string;
  } | null;
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
    ...codeModules.map((codeModule) => ({
      source: codeModule.source,
      filePath: `file:///node_modules/${codeModule.source}/index.d.ts`,
      content: codeModule.type_declarations
    }))
  ];
  return {
    allowedImportSources: new Set(
      codeModules.map((codeModule) => codeModule.source)
    ),
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
