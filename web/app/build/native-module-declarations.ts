import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import ts from 'typescript';
import type { Plugin } from 'vite';

export const NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID =
  'virtual:1flowbase-native-module-declarations';

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID}`;
const UNRESOLVED_DECLARATION_DIAGNOSTIC_CODES = new Set([
  2307, // Cannot find module.
  2688, // Cannot find type definition file.
  6053, // File not found.
  7016 // No declaration file for module.
]);

export interface CollectedNativeModuleDeclaration {
  content: string;
  filePath: string;
  source: string;
}

interface CollectedDeclarationGraph {
  extraLibs: CollectedNativeModuleDeclaration[];
  watchedFiles: string[];
}

export function nativeModuleDeclarationsPlugin({
  moduleSources,
  projectRoot
}: {
  moduleSources: readonly string[];
  projectRoot: string;
}): Plugin {
  let graph: CollectedDeclarationGraph | undefined;

  return {
    name: '1flowbase-native-module-declarations',
    enforce: 'pre',
    resolveId(id) {
      return id === NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : undefined;
    },
    load(id) {
      if (id !== RESOLVED_VIRTUAL_ID) return undefined;
      graph ??= collectNativeModuleDeclarationGraph({
        moduleSources,
        projectRoot
      });
      for (const watchedFile of graph.watchedFiles) {
        this.addWatchFile(watchedFile);
      }
      return `export default ${JSON.stringify(graph.extraLibs)};`;
    },
    watchChange() {
      graph = undefined;
    }
  };
}

export function collectNativeModuleDeclarations({
  moduleSources,
  projectRoot
}: {
  moduleSources: readonly string[];
  projectRoot: string;
}): CollectedNativeModuleDeclaration[] {
  return collectNativeModuleDeclarationGraph({ moduleSources, projectRoot })
    .extraLibs;
}

function collectNativeModuleDeclarationGraph({
  moduleSources,
  projectRoot
}: {
  moduleSources: readonly string[];
  projectRoot: string;
}): CollectedDeclarationGraph {
  const compilerOptions: ts.CompilerOptions = {
    allowSyntheticDefaultImports: true,
    esModuleInterop: true,
    jsx: ts.JsxEmit.ReactJSX,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true,
    target: ts.ScriptTarget.ES2022,
    types: []
  };
  const containingFile = path.join(
    projectRoot,
    'src/__native-module-declarations__.tsx'
  );
  const rootNames = moduleSources.map((moduleSource) => {
    const resolution = ts.resolveModuleName(
      moduleSource,
      containingFile,
      compilerOptions,
      ts.sys
    ).resolvedModule;
    if (!resolution) {
      throw new Error(
        `[native-module-declarations] Cannot resolve declarations for '${moduleSource}' from '${projectRoot}'.`
      );
    }
    if (!isDeclarationFile(resolution.resolvedFileName)) {
      throw new Error(
        `[native-module-declarations] '${moduleSource}' resolved to '${resolution.resolvedFileName}', not a declaration file.`
      );
    }
    return resolution.resolvedFileName;
  });

  const program = ts.createProgram(rootNames, compilerOptions);
  const unresolvedDiagnostics = ts
    .getPreEmitDiagnostics(program)
    .filter((diagnostic) =>
      UNRESOLVED_DECLARATION_DIAGNOSTIC_CODES.has(diagnostic.code)
    );
  if (unresolvedDiagnostics.length > 0) {
    throw new Error(
      `[native-module-declarations] Declaration graph is incomplete:\n${ts.formatDiagnosticsWithColorAndContext(
        unresolvedDiagnostics,
        {
          getCanonicalFileName: (fileName) => fileName,
          getCurrentDirectory: () => projectRoot,
          getNewLine: () => '\n'
        }
      )}`
    );
  }

  const physicalFiles = new Set(
    program
      .getSourceFiles()
      .filter(
        (sourceFile) =>
          sourceFile.isDeclarationFile &&
          normalizePath(sourceFile.fileName).includes('/node_modules/')
      )
      .map((sourceFile) => sourceFile.fileName)
  );
  for (const declarationFile of [...physicalFiles]) {
    physicalFiles.add(findOwningPackageJson(declarationFile));
  }

  const byVirtualPath = new Map<string, CollectedNativeModuleDeclaration>();
  for (const physicalFile of [...physicalFiles].sort()) {
    const filePath = toVirtualNodeModulesPath(physicalFile);
    const content = readFileSync(physicalFile, 'utf8');
    const existing = byVirtualPath.get(filePath);
    if (existing && existing.content !== content) {
      throw new Error(
        `[native-module-declarations] Multiple resolved files map to '${filePath}' with different contents.`
      );
    }
    byVirtualPath.set(filePath, {
      content,
      filePath,
      source: packageSourceFromVirtualPath(filePath)
    });
  }

  return {
    extraLibs: [...byVirtualPath.values()].sort((left, right) =>
      left.filePath.localeCompare(right.filePath)
    ),
    watchedFiles: [...physicalFiles]
  };
}

function findOwningPackageJson(fileName: string): string {
  let directory = path.dirname(fileName);
  while (true) {
    const candidate = path.join(directory, 'package.json');
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
    const parent = path.dirname(directory);
    if (parent === directory) {
      throw new Error(
        `[native-module-declarations] Cannot find package.json for '${fileName}'.`
      );
    }
    directory = parent;
  }
}

function toVirtualNodeModulesPath(fileName: string): string {
  const normalized = normalizePath(fileName);
  const marker = '/node_modules/';
  const markerIndex = normalized.lastIndexOf(marker);
  if (markerIndex < 0) {
    throw new Error(
      `[native-module-declarations] Declaration '${fileName}' is not inside node_modules.`
    );
  }
  return `file:///node_modules/${normalized.slice(markerIndex + marker.length)}`;
}

function packageSourceFromVirtualPath(filePath: string): string {
  const relativePath = filePath.slice('file:///node_modules/'.length);
  const segments = relativePath.split('/');
  const packageName = segments[0];
  if (!packageName) {
    throw new Error(
      `[native-module-declarations] Cannot identify package for '${filePath}'.`
    );
  }
  if (packageName === '@types') {
    const typePackageName = segments[1];
    if (!typePackageName) {
      throw new Error(
        `[native-module-declarations] Cannot identify type package for '${filePath}'.`
      );
    }
    return typePackageName.includes('__')
      ? `@${typePackageName.replace('__', '/')}`
      : typePackageName;
  }
  if (packageName.startsWith('@') && !segments[1]) {
    throw new Error(
      `[native-module-declarations] Cannot identify scoped package for '${filePath}'.`
    );
  }
  return packageName.startsWith('@')
    ? `${packageName}/${segments[1]}`
    : packageName;
}

function isDeclarationFile(fileName: string): boolean {
  return /\.d\.(?:c|m)?ts$/.test(fileName);
}

function normalizePath(fileName: string): string {
  return fileName.replaceAll('\\', '/');
}
