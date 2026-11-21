// Compiler-owned edges are authoritative. Never infer imports from bundled
// strings: editor declarations contain valid-looking import statements too.
type Chunk = {
  type: 'chunk';
  fileName: string;
  name: string;
  modules: Record<string, unknown>;
  imports: readonly string[];
  dynamicImports: readonly string[];
  isEntry?: boolean;
  isDynamicEntry?: boolean;
  viteMetadata?: {
    importedCss: Set<string>;
    importedAssets: Set<string>;
  };
};
type Asset = { type: 'asset'; fileName: string };
type Bundle = Record<string, Chunk | Asset>;

const sceneOwners = {
  discovery: ['/src/bootstrap.ts'],
  signIn: ['/features/auth/pages/SignInPage.tsx'],
  shell: ['/app-shell/AppShellFrame.tsx'],
  browse: ['/features/frontstage/pages/FrontStagePage.tsx'],
  studio: ['/features/frontstage/components/jsx-studio/FrontstageJsxStudioDrawer.tsx'],
  loginStudio: ['/features/settings/components/auth-center/LoginEntryUiBlockStudio.tsx']
} as const;

export function staticChunkClosure(
  chunks: ReadonlyMap<string, { imports: readonly string[] }>,
  roots: readonly string[]
): string[] {
  const pending = [...roots];
  const visited = new Set<string>();
  while (pending.length) {
    const file = pending.pop()!;
    if (visited.has(file) || !chunks.has(file)) continue;
    visited.add(file);
    pending.push(...chunks.get(file)!.imports);
  }
  return [...visited].sort();
}

export function collectScenarioDemandManifest(bundle: Bundle) {
  const chunks = Object.values(bundle).filter(
    (item): item is Chunk => item.type === 'chunk'
  );
  const byFile = new Map(chunks.map((chunk) => [chunk.fileName, chunk]));
  const owners = new Map(chunks.map((chunk) => [
    chunk.fileName,
    Object.keys(chunk.modules).map((id) => id.replaceAll('\\', '/'))
  ]));
  const scenes = Object.fromEntries(Object.entries(sceneOwners).map(([scene, suffixes]) => {
    const roots = chunks.filter((chunk) => owners.get(chunk.fileName)!.some(
      (id) => suffixes.some((suffix) => id.endsWith(suffix))
    )).map((chunk) => chunk.fileName).sort();
    return [scene, { roots, staticClosure: staticChunkClosure(byFile, roots) }];
  }));

  // Genuine runtime compiler Workers remain legal. Only authoring modules are
  // excluded; a published/custom Block must retain its actual runtime demand.
  for (const scene of ['signIn', 'shell', 'browse']) {
    for (const file of scenes[scene]!.staticClosure) {
      for (const id of owners.get(file)!) {
        if (
          /\/components\/jsx-studio\//u.test(id) ||
          /\/lib\/jsx-studio\/editor-projection\.ts$/u.test(id) ||
          id.includes('virtual:1flowbase-native-module-declarations') ||
          /\/node_modules\/monaco-editor\//u.test(id)
        ) {
          throw new Error(`[scenario-demand] ${scene} eagerly includes authoring module ${id} in ${file}`);
        }
      }
    }
  }

  return {
    version: 2,
    scenes,
    chunks: chunks.map((chunk) => ({
      file: chunk.fileName,
      name: chunk.name,
      imports: [...chunk.imports].sort(),
      dynamicImports: [...chunk.dynamicImports].sort(),
      css: [...(chunk.viteMetadata?.importedCss ?? [])].sort(),
      assets: [...(chunk.viteMetadata?.importedAssets ?? [])].sort(),
      moduleCount: Object.keys(chunk.modules).length,
      isEntry: Boolean(chunk.isEntry),
      isDynamicEntry: Boolean(chunk.isDynamicEntry)
    })).sort((a, b) => a.file.localeCompare(b.file)),
    // Worker outputs and hashed declaration resources are emitted assets rather
    // than ordinary static imports. Runtime network capture resolves their demand.
    assets: Object.values(bundle).filter((item) => item.type === 'asset')
      .map((item) => item.fileName).sort()
  };
}
