import { createHash } from 'node:crypto';

export interface CollectedNativeModuleDeclaration {
  content: string;
  filePath: string;
  source: string;
}

export interface NativeDeclarationGraph {
  extraLibs: CollectedNativeModuleDeclaration[];
  roots: Record<string, string[]>;
  dependencies: Record<string, string[]>;
}

// Keep primary entry closures exact, without fragmenting them for every deep
// import. Unowned files remain in real-package partitions; the quotient graph
// closes over dependencies of *all* co-loaded files, not just the requested one.
export function createDeclarationResources(
  graph: NativeDeclarationGraph,
  primaryRoots: readonly string[] = Object.keys(graph.roots)
) {
  const files = new Map(graph.extraLibs.map((file) => [file.filePath, file]));
  const fileClosure = (entries: Iterable<string>) =>
    closure(entries, (name) => {
      if (!files.has(name))
        throw new Error(`Missing declaration graph dependency: ${name}`);
      return graph.dependencies[name] ?? [];
    });
  const reachable = fileClosure(Object.values(graph.roots).flat());
  for (const file of graph.extraLibs) {
    if (!reachable.has(file.filePath))
      throw new Error(`Unreachable declaration graph file: ${file.filePath}`);
  }

  const owners = new Map<string, string[]>();
  for (const root of [...new Set(primaryRoots)].sort()) {
    if (!Object.hasOwn(graph.roots, root))
      throw new Error(`Missing primary declaration root: ${root}`);
    for (const name of fileClosure(graph.roots[root]!)) {
      const names = owners.get(name) ?? [];
      names.push(root);
      owners.set(name, names);
    }
  }
  const groups = new Map<string, CollectedNativeModuleDeclaration[]>();
  for (const file of graph.extraLibs) {
    const primaryOwners = owners.get(file.filePath);
    // Use physical package identity, not the presentation/source label (which
    // can collapse @types/foo with foo), and retain every residual subpath.
    const key = JSON.stringify(
      primaryOwners
        ? ['primary', primaryOwners]
        : ['package', packageFromVirtualPath(file.filePath)]
    );
    const group = groups.get(key) ?? [];
    group.push(file);
    groups.set(key, group);
  }

  const resources: Record<string, string> = {};
  const fileResources = new Map<string, string>();
  for (const files of groups.values()) {
    const content = JSON.stringify(
      files.sort((a, b) => a.filePath.localeCompare(b.filePath))
    );
    const hash = createHash('sha256').update(content).digest('hex');
    resources[hash] = content;
    for (const file of files) fileResources.set(file.filePath, hash);
  }
  const resourceFor = (file: string) => {
    const resource = fileResources.get(file);
    if (!resource)
      throw new Error(`Missing declaration graph dependency: ${file}`);
    return resource;
  };
  const dependencies = new Map<string, Set<string>>();
  for (const [from, targets] of Object.entries(graph.dependencies)) {
    const source = resourceFor(from);
    const edges = dependencies.get(source) ?? new Set<string>();
    for (const target of targets) edges.add(resourceFor(target));
    dependencies.set(source, edges);
  }
  const roots = Object.fromEntries(
    Object.entries(graph.roots).map(([root, entries]) => [
      root,
      [
        ...closure(
          entries.map(resourceFor),
          (hash) => dependencies.get(hash) ?? []
        )
      ].sort()
    ])
  );
  return { version: 1 as const, roots, resources };
}

function closure(
  entries: Iterable<string>,
  dependencies: (entry: string) => Iterable<string>
): Set<string> {
  const visited = new Set<string>();
  const pending = [...entries];
  while (pending.length) {
    const name = pending.pop()!;
    if (visited.has(name)) continue;
    visited.add(name);
    pending.push(...dependencies(name));
  }
  return visited;
}

function packageFromVirtualPath(filePath: string): string {
  const name = /^file:\/\/\/node_modules\/((?:@[^/]+\/)?[^/]+)\//u.exec(
    filePath
  )?.[1];
  if (!name)
    throw new Error(`Cannot identify declaration package: ${filePath}`);
  return name;
}
