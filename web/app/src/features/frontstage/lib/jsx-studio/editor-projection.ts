import declarationIndex from 'virtual:1flowbase-native-module-declarations';

import type {
  BlockSourceDeclarationLoader,
  BlockSourceExtraLib
} from '../../../../shared/code-block/extra-lib';
import type { FrontstageBlockTypeDeclaration } from '../../api/block-catalog';
import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import {
  FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
  frontstageNativeDeclarationLoader
} from '../native-modules/editor-declarations';
import { createNativeDeclarationLoader } from '../native-modules/editor-declarations/loader';
import { FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS } from '../native-modules/registry';
import { FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS } from './native-react-editor-contract';

const HOST_NATIVE_REACT_MODULE_SOURCES = new Set(
  FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS.map(
    ({ module_source }) => module_source
  )
);

export interface FrontstageJsxEditorProjection {
  allowedImportSources: ReadonlySet<string>;
  contextComment: string;
  monacoExtraLibs: BlockSourceExtraLib[];
  declarationLoader: BlockSourceDeclarationLoader;
}

export function createFrontstageJsxEditorProjection({
  catalogEntry,
  catalogEntries = [],
  typeDeclarations = []
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[];
  typeDeclarations?: readonly FrontstageBlockTypeDeclaration[];
}): FrontstageJsxEditorProjection {
  const allowedImportSources = new Set(HOST_NATIVE_REACT_MODULE_SOURCES);
  const allCatalogEntries =
    catalogEntry && !catalogEntries.includes(catalogEntry)
      ? [catalogEntry, ...catalogEntries]
      : catalogEntries;

  for (const entry of allCatalogEntries) {
    if (entry.raw?.disable_reason !== null) continue;
    for (const module of entry.raw?.code_modules ?? []) {
      if (
        module.binding === 'fetched' &&
        module.source.trim().length > 0 &&
        !HOST_NATIVE_REACT_MODULE_SOURCES.has(module.source)
      ) {
        allowedImportSources.add(module.source);
      }
    }
  }

  const pluginExtraLibsBySource = new Map<string, BlockSourceExtraLib>();
  for (const declaration of typeDeclarations) {
    const source = declaration.source.trim();
    const content = declaration.type_declarations.trimEnd();
    if (
      declaration.binding !== 'fetched' ||
      !source ||
      !content.trim() ||
      HOST_NATIVE_REACT_MODULE_SOURCES.has(source) ||
      pluginExtraLibsBySource.has(source)
    ) {
      continue;
    }
    allowedImportSources.add(source);
    pluginExtraLibsBySource.set(source, {
      source,
      filePath: `file:///node_modules/${source}/index.d.ts`,
      content
    });
  }

  const pluginExtraLibs = [...pluginExtraLibsBySource.values()];
  const monacoExtraLibs = [
    ...FRONTSTAGE_NATIVE_REACT_MONACO_EXTRA_LIBS,
    ...pluginExtraLibs
  ];
  const declarationLoader =
    pluginExtraLibs.length > 0
      ? createNativeDeclarationLoader(declarationIndex, [
          ...FRONTSTAGE_NATIVE_REACT_MODULE_EXTRA_LIBS,
          ...pluginExtraLibs
        ])
      : frontstageNativeDeclarationLoader;

  return {
    allowedImportSources,
    contextComment: createFrontstageContextComment(),
    monacoExtraLibs,
    declarationLoader
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
