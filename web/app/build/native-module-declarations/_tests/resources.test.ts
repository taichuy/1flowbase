import { describe, expect, test } from 'vitest';

import { createDeclarationResources } from '../resources';

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
