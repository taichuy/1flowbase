import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expect, test } from 'vitest';
import { collectNativeModuleDeclarationGraph } from '../graph';
import { createDeclarationResources } from '../resources';

test('TypeScript resolution supplies cross-package, triple-slash and types edges with original virtual paths', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'native-declaration-graph-'));
  const put = (name: string, text: string) => {
    const file = path.join(root, 'node_modules', name);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, text);
  };
  try {
    for (const name of ['alpha', 'beta', '@types/bridge', 'unrelated']) {
      put(
        `${name}/package.json`,
        JSON.stringify({ name, types: 'index.d.ts' })
      );
    }
    put(
      'alpha/index.d.ts',
      `/// <reference path="./ambient.d.ts" />\n/// <reference types="bridge" />\nexport { B } from 'beta';\nexport type A = import('beta').B;`
    );
    put('alpha/ambient.d.ts', 'declare interface Ambient { label: string }');
    put('@types/bridge/index.d.ts', `export { B as Bridge } from 'beta';`);
    put(
      'beta/index.d.ts',
      `export interface B { value: string }\nexport * from './nested';`
    );
    put('beta/nested/index.d.ts', `export { B } from '../index';`);
    put(
      'beta/nested/package.json',
      JSON.stringify({ types: 'index.d.ts', type: 'module' })
    );
    put('unrelated/index.d.ts', 'export interface Unrelated { other: number }');
    const graph = collectNativeModuleDeclarationGraph({
      projectRoot: root,
      moduleSources: ['alpha', 'unrelated']
    });
    const index = createDeclarationResources(graph, ['alpha']);
    const libs = index.roots.alpha!.flatMap((id) =>
      JSON.parse(index.resources[id]!)
    ) as { filePath: string }[];
    const paths = libs.map((lib) => lib.filePath);
    expect(paths).toEqual(
      expect.arrayContaining([
        'file:///node_modules/alpha/index.d.ts',
        'file:///node_modules/alpha/ambient.d.ts',
        'file:///node_modules/@types/bridge/index.d.ts',
        'file:///node_modules/@types/bridge/package.json',
        'file:///node_modules/beta/index.d.ts',
        'file:///node_modules/beta/nested/index.d.ts',
        'file:///node_modules/beta/package.json',
        'file:///node_modules/beta/nested/package.json'
      ])
    );
    expect(paths.some((file) => file.includes('/unrelated/'))).toBe(false);
    expect(
      Object.values(index.resources).flatMap((body) => JSON.parse(body))
    ).toHaveLength(graph.extraLibs.length);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
