import { createHash } from 'node:crypto';
import { describe, expect, test } from 'vitest';

import {
  createDeclarationResources,
  type CollectedNativeModuleDeclaration,
  type NativeDeclarationGraph
} from '../resources';

const file = (filePath: string, content = filePath) => ({
  filePath,
  content,
  source: 'deliberately-not-the-dependency-boundary'
});

describe('declaration resources', () => {
  test('loads actual transitive cross-package closure, cycles and package metadata, not source groups', () => {
    const files = ['a', 'b', 'c', 'package-a', 'unused'].map((name) =>
      file(name)
    );
    const graph = {
      extraLibs: files,
      roots: { alpha: ['a'], beta: ['unused'] },
      dependencies: {
        a: ['b', 'package-a'],
        b: ['c'],
        c: ['b'],
        'package-a': [],
        unused: []
      }
    };
    const result = createDeclarationResources(graph);
    const selected = result.roots.alpha.flatMap((id) =>
      JSON.parse(result.resources[id]!)
    );
    expect(
      selected.map((entry: { filePath: string }) => entry.filePath).sort()
    ).toEqual(['a', 'b', 'c', 'package-a']);
    expect(Object.keys(result.resources)).toHaveLength(2);
    expect(
      Object.values(result.resources).flatMap((body) => JSON.parse(body))
    ).toHaveLength(files.length);
  });

  test('a changed unrelated declaration leaves the demanded content hash unchanged', () => {
    const make = (content: string) =>
      createDeclarationResources({
        extraLibs: [file('a'), file('b', content), file('shared')],
        roots: { alpha: ['a'], beta: ['b'] },
        dependencies: { a: ['shared'], b: ['shared'], shared: [] }
      });
    const before = make('old');
    const after = make('new');
    expect(after.roots.alpha).toEqual(before.roots.alpha);
    expect(after.roots.beta).not.toEqual(before.roots.beta);
  });

  test('fails rather than silently dropping unresolved edges or unreachable declarations', () => {
    expect(() =>
      createDeclarationResources({
        extraLibs: [file('a')],
        roots: { a: ['a'] },
        dependencies: { a: ['missing'] }
      })
    ).toThrow(/missing/);
    expect(() =>
      createDeclarationResources({
        extraLibs: [file('orphan')],
        roots: {},
        dependencies: {}
      })
    ).toThrow(/orphan/);
  });
});

const virtualPath = (name: string) => `file:///node_modules/${name}`;
const partitionFixture = (): NativeDeclarationGraph => ({
  extraLibs: [
    'alpha/index.d.ts',
    'alpha/package.json',
    'alpha/deep-a.d.ts',
    'alpha/deep-b.d.ts',
    'peer/index.d.ts',
    'shared/index.d.ts',
    '@scope/bridge/index.d.ts',
    '@scope/bridge/package.json',
    '@types/bridge/index.d.ts',
    '@types/bridge/package.json',
    'tail/index.d.ts',
    'unused/index.d.ts'
  ].map((name) => file(virtualPath(name))),
  roots: Object.fromEntries(
    Object.entries({
      alpha: ['alpha/index.d.ts'],
      'alpha/alias': ['alpha/index.d.ts'],
      'alpha/deep-a': ['alpha/deep-a.d.ts'],
      'alpha/deep-b': ['alpha/deep-b.d.ts'],
      peer: ['peer/index.d.ts'],
      unused: ['unused/index.d.ts']
    }).map(([root, entries]) => [root, entries.map(virtualPath)])
  ),
  dependencies: Object.fromEntries(
    Object.entries({
      'alpha/index.d.ts': ['shared/index.d.ts', 'alpha/package.json'],
      'peer/index.d.ts': ['shared/index.d.ts'],
      'alpha/deep-a.d.ts': ['alpha/package.json'],
      // Only the co-loaded sibling introduces bridge and its transitive cycle.
      'alpha/deep-b.d.ts': ['@scope/bridge/index.d.ts'],
      '@scope/bridge/index.d.ts': [
        'tail/index.d.ts',
        '@scope/bridge/package.json'
      ],
      'tail/index.d.ts': [
        '@scope/bridge/index.d.ts',
        '@types/bridge/index.d.ts'
      ],
      '@types/bridge/index.d.ts': ['@types/bridge/package.json']
    }).map(([from, targets]) => [virtualPath(from), targets.map(virtualPath)])
  )
});
const selectedFiles = (
  index: ReturnType<typeof createDeclarationResources>,
  root?: string
): CollectedNativeModuleDeclaration[] =>
  (root === undefined
    ? Object.values(index.resources)
    : index.roots[root]!.map((id) => index.resources[id]!)
  )
    .flatMap((body) => JSON.parse(body))
    .sort((a, b) => a.filePath.localeCompare(b.filePath));

// Independent per-file oracle; it knows nothing about resource partitions.
const exactFiles = (graph: NativeDeclarationGraph, root: string) => {
  const visited = new Set<string>();
  const visit = (name: string) => {
    if (visited.has(name)) return;
    visited.add(name);
    for (const dependency of graph.dependencies[name] ?? []) visit(dependency);
  };
  graph.roots[root]!.forEach(visit);
  return graph.extraLibs
    .filter((file) => visited.has(file.filePath))
    .sort((a, b) => a.filePath.localeCompare(b.filePath));
};

describe('public-entry partitions and residual package graph', () => {
  test('keeps exact primary closures/aliases without importing unused siblings or other entries', () => {
    const graph = partitionFixture();
    const index = createDeclarationResources(graph, ['alpha', 'peer']);
    for (const root of ['alpha', 'alpha/alias', 'peer']) {
      expect(selectedFiles(index, root)).toEqual(exactFiles(graph, root));
    }
    expect(index.roots['alpha/alias']).toEqual(index.roots.alpha);
    expect(
      selectedFiles(index, 'alpha').map((file) => file.filePath)
    ).not.toContain(virtualPath('alpha/deep-b.d.ts'));
    // All-root signatures would unnecessarily split the two residual siblings.
    expect(index.roots['alpha/deep-a']).toEqual(index.roots['alpha/deep-b']);
    expect(index.roots.alpha!.length).toBeLessThan(
      createDeclarationResources(graph).roots.alpha!.length
    );
  });

  test('closes over dependencies of co-loaded residual subpaths, including cross-package cycles and metadata', () => {
    const graph = partitionFixture();
    const index = createDeclarationResources(graph, ['alpha', 'peer']);
    const selected = selectedFiles(index, 'alpha/deep-a').map(
      (file) => file.filePath
    );
    expect(selected).toEqual(
      expect.arrayContaining(
        [
          'alpha/deep-a.d.ts',
          'alpha/deep-b.d.ts',
          '@scope/bridge/index.d.ts',
          '@scope/bridge/package.json',
          '@types/bridge/index.d.ts',
          '@types/bridge/package.json',
          'tail/index.d.ts'
        ].map(virtualPath)
      )
    );
    expect(selected).not.toContain(virtualPath('unused/index.d.ts'));
    // A naive mapping of the originally requested file closure omits bridge.
    expect(
      exactFiles(graph, 'alpha/deep-a').map((file) => file.filePath)
    ).not.toContain(virtualPath('@scope/bridge/index.d.ts'));
    for (const root of Object.keys(graph.roots)) {
      const loaded = new Set(
        selectedFiles(index, root).map((file) => file.filePath)
      );
      expect([...loaded]).toEqual(expect.arrayContaining(graph.roots[root]!));
      for (const file of loaded) {
        for (const dependency of graph.dependencies[file] ?? []) {
          expect(loaded.has(dependency)).toBe(true);
        }
      }
    }
  });

  test('retains every file exactly once, physical scoped/@types package identities, and stable content hashes', () => {
    const graph = partitionFixture();
    const before = createDeclarationResources(graph, ['alpha', 'peer']);
    expect(selectedFiles(before)).toEqual(
      [...graph.extraLibs].sort((a, b) => a.filePath.localeCompare(b.filePath))
    );
    const bridgePackages = Object.values(before.resources).map((body) =>
      (JSON.parse(body) as CollectedNativeModuleDeclaration[]).map(
        (file) => file.filePath
      )
    );
    expect(bridgePackages).toContainEqual([
      virtualPath('@scope/bridge/index.d.ts'),
      virtualPath('@scope/bridge/package.json')
    ]);
    expect(bridgePackages).toContainEqual([
      virtualPath('@types/bridge/index.d.ts'),
      virtualPath('@types/bridge/package.json')
    ]);
    for (const [hash, body] of Object.entries(before.resources)) {
      expect(hash).toMatch(/^[a-f0-9]{64}$/u);
      expect(hash).toBe(createHash('sha256').update(body).digest('hex'));
    }
    const reordered = {
      extraLibs: [...graph.extraLibs].reverse(),
      roots: Object.fromEntries(Object.entries(graph.roots).reverse()),
      dependencies: Object.fromEntries(
        Object.entries(graph.dependencies)
          .reverse()
          .map(([file, edges]) => [file, [...edges].reverse()])
      )
    };
    expect(
      createDeclarationResources(reordered, ['peer', 'alpha', 'peer'])
    ).toEqual(before);
    graph.extraLibs.find(
      (file) => file.filePath === virtualPath('alpha/deep-b.d.ts')
    )!.content += 'changed';
    const changed = createDeclarationResources(graph, ['alpha', 'peer']);
    expect(changed.roots.alpha).toEqual(before.roots.alpha);
    expect(changed.roots.peer).toEqual(before.roots.peer);
    expect(changed.roots['alpha/deep-a']).not.toEqual(
      before.roots['alpha/deep-a']
    );
  });

  test('does not let residual grouping conceal missing dependencies or invalid primary roots', () => {
    const graph = partitionFixture();
    expect(() => createDeclarationResources(graph, ['missing'])).toThrow(
      /Missing primary/
    );
    graph.dependencies[virtualPath('alpha/deep-b.d.ts')]!.push(
      virtualPath('missing/index.d.ts')
    );
    expect(() => createDeclarationResources(graph, ['alpha', 'peer'])).toThrow(
      /missing\/index/
    );
  });
});
