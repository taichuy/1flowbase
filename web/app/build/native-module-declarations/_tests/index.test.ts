import { createHash } from 'node:crypto';
import { describe, expect, test } from 'vitest';

import { compactDeclarationIndex, serializeDeclarationIndex } from '../index';
import { createDeclarationResources } from '../resources';

type Index = ReturnType<typeof createDeclarationResources>;
const hash = (value: string) =>
  createHash('sha256').update(value).digest('hex');
const url = (hash: string) => `/base/assets/native-declarations/${hash}.json`;
const evaluate = (source: string): Index =>
  new Function(source.replace('export default', 'return'))();
const urls = (index: Index) =>
  Object.fromEntries(
    Object.keys(index.resources).map((hash) => [hash, url(hash)])
  );

describe('compact declaration index', () => {
  test('roundtrips exact closures, aliases, cycles, metadata and every resource URL', () => {
    const names = ['alpha', 'beta', 'shared', 'package.json', 'other'];
    const index = createDeclarationResources({
      extraLibs: names.map((name) => ({
        filePath: `file:///node_modules/${name}`,
        content: name,
        source: 'not-a-resource-boundary'
      })),
      roots: Object.fromEntries(
        [
          ['alpha', 'alpha'],
          ['alpha/entry', 'alpha'],
          ['@scope/beta', 'beta'],
          ['other', 'other'],
          ['__proto__', 'other']
        ].map(([name, entry]) => [name, [`file:///node_modules/${entry}`]])
      ),
      dependencies: Object.fromEntries(
        Object.entries({
          alpha: ['beta'],
          beta: ['shared', 'package.json'],
          shared: ['beta'],
          'package.json': [],
          other: []
        }).map(([name, dependencies]) => [
          `file:///node_modules/${name}`,
          dependencies.map((name) => `file:///node_modules/${name}`)
        ])
      )
    });
    const actual = evaluate(
      serializeDeclarationIndex(index, (hash) => JSON.stringify(url(hash)))
    );
    expect(actual).toEqual({ ...index, resources: urls(index) });
    expect(actual.roots['alpha/entry']).toBe(actual.roots.alpha);
    expect(Object.hasOwn(actual.roots, '__proto__')).toBe(true);
    for (const root of Object.keys(index.roots)) {
      const contents = (ids: string[]) =>
        ids.flatMap((id) => JSON.parse(index.resources[id]!));
      expect(contents(actual.roots[root]!)).toEqual(
        contents(index.roots[root]!)
      );
    }
  });

  test('uses stable numeric IDs and preserves ordered and empty closures', () => {
    const a = hash('a');
    const b = hash('b');
    const index: Index = {
      version: 1,
      roots: { forward: [a, b], reverse: [b, a], empty: [], alias: [a, b] },
      resources: { [b]: 'B', [a]: 'A' }
    };
    const compact = compactDeclarationIndex(index);
    expect(compact.hashes).toEqual([a, b].sort());
    expect(compact.groups).toHaveLength(3);
    expect(compact.groups[0]![1]).toEqual(['forward', 'alias']);
    expect(
      compactDeclarationIndex({
        ...index,
        resources: { [a]: 'A', [b]: 'B' }
      })
    ).toEqual(compact);
    expect(
      evaluate(
        serializeDeclarationIndex(index, (hash) => JSON.stringify(url(hash)))
      )
    ).toEqual({ ...index, resources: urls(index) });
  });

  test('dense alias closures serialize long hashes only in the inventory and URLs', () => {
    const hashes = Array.from({ length: 64 }, (_, id) => hash(String(id)));
    const index: Index = {
      version: 1,
      resources: Object.fromEntries(hashes.map((hash) => [hash, '{}'])),
      roots: Object.fromEntries(
        Array.from({ length: 512 }, (_, root) => [
          `@scope/package/entry-${root}`,
          hashes.slice(root % 8)
        ])
      )
    };
    const compact = compactDeclarationIndex(index);
    expect(compact.groups).toHaveLength(8);
    expect(compact.groups.every(([ids]) => ids.every(Number.isInteger))).toBe(
      true
    );
    const payload = JSON.stringify(compact);
    const source = serializeDeclarationIndex(index, (hash) =>
      JSON.stringify(url(hash))
    );
    for (const hash of hashes) {
      expect(payload.split(hash)).toHaveLength(2);
      // One inventory literal plus one content-addressed URL, never per root.
      expect(source.split(hash)).toHaveLength(3);
    }
    expect(source.length).toBeLessThan(JSON.stringify(index.roots).length / 10);
    const actual = evaluate(source);
    expect(actual).toEqual({ ...index, resources: urls(index) });
    expect(actual.roots['@scope/package/entry-0']).toBe(
      actual.roots['@scope/package/entry-8']
    );
  });

  test('rejects a missing resource instead of serializing an undefined ID', () => {
    expect(() =>
      compactDeclarationIndex({
        version: 1,
        roots: { entry: ['missing'] },
        resources: {}
      })
    ).toThrow(/Missing declaration resource 'missing' for 'entry'/);
  });
});
