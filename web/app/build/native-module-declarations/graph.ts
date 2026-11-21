import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
// Monaco .d.ts graph resolution requires the JS TypeScript Compiler API while workspace `typescript` is native TS 7.
import ts from 'typescript-legacy';
import type {
  CollectedNativeModuleDeclaration,
  NativeDeclarationGraph
} from './resources';

const UNRESOLVED_DECLARATION_DIAGNOSTIC_CODES = new Set([
  2307, 2688, 6053, 7016
]);

export function collectNativeModuleDeclarationGraph({
  moduleSources,
  projectRoot
}: {
  moduleSources: readonly string[];
  projectRoot: string;
}): NativeDeclarationGraph & { watchedFiles: string[] } {
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

  const edges = new Map<string, Set<string>>();
  const host = ts.createCompilerHost(compilerOptions);
  const addEdge = (from: string, to: string) => {
    const dependencies = edges.get(from) ?? new Set<string>();
    dependencies.add(to);
    edges.set(from, dependencies);
  };
  // Capture the same resolutions used by the program, including import types,
  // re-exports, per-import resolution modes and cross-package dependencies.
  host.resolveModuleNameLiterals = (
    literals,
    containingFile,
    redirectedReference,
    options,
    sourceFile
  ) =>
    literals.map((literal) => {
      const resolution = ts.resolveModuleName(
        literal.text,
        containingFile,
        options,
        host,
        undefined,
        redirectedReference,
        ts.getModeForUsageLocation(sourceFile, literal, options)
      );
      if (resolution.resolvedModule)
        addEdge(containingFile, resolution.resolvedModule.resolvedFileName);
      return resolution;
    });
  host.resolveTypeReferenceDirectiveReferences = (
    references,
    containingFile,
    redirectedReference,
    options,
    sourceFile
  ) =>
    references.map((reference) => {
      const resolution = ts.resolveTypeReferenceDirective(
        typeof reference === 'string' ? reference : reference.fileName,
        containingFile,
        options,
        host,
        redirectedReference,
        undefined,
        ts.getModeForFileReference(reference, sourceFile?.impliedNodeFormat)
      );
      if (resolution.resolvedTypeReferenceDirective?.resolvedFileName)
        addEdge(
          containingFile,
          resolution.resolvedTypeReferenceDirective.resolvedFileName
        );
      return resolution;
    });
  const program = ts.createProgram(rootNames, compilerOptions, host);
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

  const physicalFiles = new Set<string>();
  for (const sourceFile of program.getSourceFiles()) {
    if (
      sourceFile.isDeclarationFile &&
      normalizePath(sourceFile.fileName).includes('/node_modules/')
    ) {
      physicalFiles.add(sourceFile.fileName);
    }
  }
  for (const sourceFile of program.getSourceFiles()) {
    if (!physicalFiles.has(sourceFile.fileName)) continue;
    for (const reference of sourceFile.referencedFiles) {
      addEdge(
        sourceFile.fileName,
        path.resolve(path.dirname(sourceFile.fileName), reference.fileName)
      );
    }
    for (const reference of sourceFile.libReferenceDirectives) {
      const name = `lib.${reference.fileName.toLowerCase()}.d.ts`;
      const library = program
        .getSourceFiles()
        .find((file) => path.basename(file.fileName) === name);
      if (!library)
        throw new Error(
          `[native-module-declarations] Missing reference lib '${name}'.`
        );
      addEdge(sourceFile.fileName, library.fileName);
    }
  }
  for (const declarationFile of [...physicalFiles]) {
    // Nested package metadata and the package root both participate in Monaco
    // resolution (types, exports and typesVersions must remain intact).
    const normalized = normalizePath(declarationFile);
    const relative = normalized.slice(
      normalized.lastIndexOf('/node_modules/') + '/node_modules/'.length
    );
    const packageSegments = relative.startsWith('@') ? 2 : 1;
    const packageRoot =
      normalized.slice(0, normalized.length - relative.length) +
      relative.split('/').slice(0, packageSegments).join('/');
    let directory = path.dirname(declarationFile);
    let found = false;
    while (directory.length >= packageRoot.length) {
      const metadata = path.join(directory, 'package.json');
      if (existsSync(metadata) && statSync(metadata).isFile()) {
        physicalFiles.add(metadata);
        addEdge(declarationFile, metadata);
        found = true;
      }
      const parent = path.dirname(directory);
      if (parent === directory) break;
      directory = parent;
    }
    if (!found)
      throw new Error(
        `[native-module-declarations] Cannot find package.json for '${declarationFile}'.`
      );
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

  const dependencies: Record<string, string[]> = {};
  for (const [from, targets] of edges) {
    const virtualFrom = toVirtualNodeModulesPath(from);
    dependencies[virtualFrom] = [
      ...new Set([
        ...(dependencies[virtualFrom] ?? []),
        ...[...targets].map(toVirtualNodeModulesPath)
      ])
    ].sort();
  }
  const defaultLibraries = program
    .getSourceFiles()
    .filter((file) => program.isSourceFileDefaultLibrary(file))
    .map((file) => toVirtualNodeModulesPath(file.fileName));
  return {
    roots: Object.fromEntries(
      moduleSources.map((source, index) => [
        source,
        [toVirtualNodeModulesPath(rootNames[index]!), ...defaultLibraries]
      ])
    ),
    dependencies,
    extraLibs: [...byVirtualPath.values()].sort((left, right) =>
      left.filePath.localeCompare(right.filePath)
    ),
    watchedFiles: [...physicalFiles]
  };
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
