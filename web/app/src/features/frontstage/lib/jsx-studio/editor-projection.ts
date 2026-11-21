import type {
  BlockSourceDeclarationLoader,
  BlockSourceExtraLib
} from '../../../../shared/code-block/extra-lib';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import { frontstageNativeDeclarationLoader } from '../native-modules/editor-declarations';
import { FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS } from '../native-modules/registry';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  contextComment: string;
  monacoExtraLibs: BlockSourceExtraLib[];
  declarationLoader: BlockSourceDeclarationLoader;
}

export function createFrontstageJsxEditorProjection({
  catalogEntry: _catalogEntry
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
}): FrontstageJsxEditorProjection {
  const monacoExtraLibs = [...FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS];
  return {
    allowedImportSources: new Set(
      FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS.map(
        ({ module_source }) => module_source
      )
    ),
    contextComment: createFrontstageContextComment(),
    monacoExtraLibs,
    declarationLoader: frontstageNativeDeclarationLoader
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
