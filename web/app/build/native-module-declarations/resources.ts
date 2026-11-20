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

// Partition by actual root reachability, not by npm package or source label.
// Every member of a resource is required by exactly the same entry roots.
// This coalesces tiny declarations without pulling unrelated roots into a flight.
export function createDeclarationResources(graph: NativeDeclarationGraph) {
  const files = new Map(graph.extraLibs.map((file) => [file.filePath, file]));
  const owners = new Map<string, string[]>();
  for (const [root, entries] of Object.entries(graph.roots).sort(([a], [b]) =>
    a.localeCompare(b)
  )) {
    const visited = new Set<string>();
    const pending = [...entries];
    while (pending.length) {
      const name = pending.pop()!;
      if (visited.has(name)) continue;
      if (!files.has(name))
        throw new Error(`Missing declaration graph dependency: ${name}`);
      visited.add(name);
      owners.set(name, [...(owners.get(name) ?? []), root]);
      pending.push(...(graph.dependencies[name] ?? []));
    }
  }
  const groups = new Map<string, CollectedNativeModuleDeclaration[]>();
  for (const file of graph.extraLibs) {
    const roots = owners.get(file.filePath);
    if (!roots?.length)
      throw new Error(`Unreachable declaration graph file: ${file.filePath}`);
    const key = JSON.stringify(roots);
    const group = groups.get(key) ?? [];
    group.push(file);
    groups.set(key, group);
  }
  const resources: Record<string, string> = {};
  const roots: Record<string, string[]> = Object.fromEntries(
    Object.keys(graph.roots).map((root) => [root, []])
  );
  for (const [key, files] of groups) {
    const content = JSON.stringify(
      files.sort((a, b) => a.filePath.localeCompare(b.filePath))
    );
    const hash = createHash('sha256').update(content).digest('hex');
    resources[hash] = content;
    for (const root of JSON.parse(key) as string[]) roots[root]!.push(hash);
  }
  for (const ids of Object.values(roots)) ids.sort();
  return { version: 1 as const, roots, resources };
}
