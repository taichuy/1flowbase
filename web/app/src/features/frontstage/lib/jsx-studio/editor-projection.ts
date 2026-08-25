import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import { FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS } from '../native-modules/registry';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  contextComment: string;
  monacoExtraLibs: BlockSourceExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  catalogEntry: _catalogEntry
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
}): FrontstageJsxEditorProjection {
  const monacoExtraLibs = [
    ...FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS,
    ...FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS
  ];
  return {
    allowedImportSources: new Set(
      FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS.map(({ source }) => source)
    ),
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
