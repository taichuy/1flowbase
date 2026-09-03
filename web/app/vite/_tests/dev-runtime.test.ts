import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, test } from 'vitest';

import {
  createDevGenerationDependencyManifest,
  createDevRuntimeError,
  devCacheIdentity,
  persistDevGenerationDependencyManifest,
  pruneDevGenerationCaches
} from '../dev-runtime';

const temporaryDirectories: string[] = [];

function createFixture() {
  const fixtureRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), '1flowbase-dev-generation-')
  );
  temporaryDirectories.push(fixtureRoot);
  const webRoot = path.join(fixtureRoot, 'web');
  const appRoot = path.join(webRoot, 'app');
  const authRoot = path.join(webRoot, 'packages', 'api-client', 'src', 'auth');
  fs.mkdirSync(path.join(appRoot, 'vite'), { recursive: true });
  fs.mkdirSync(authRoot, { recursive: true });
  fs.writeFileSync(
    path.join(webRoot, 'pnpm-lock.yaml'),
    'lockfileVersion: 9\n'
  );
  fs.writeFileSync(path.join(webRoot, 'package.json'), '{"name":"web"}\n');
  fs.writeFileSync(path.join(appRoot, 'package.json'), '{"name":"app"}\n');
  fs.writeFileSync(
    path.join(appRoot, 'vite.config.ts'),
    'export default {};\n'
  );
  fs.writeFileSync(
    path.join(appRoot, 'vite', 'dev-runtime.ts'),
    '// runtime\n'
  );
  fs.writeFileSync(
    path.join(webRoot, 'packages', 'api-client', 'package.json'),
    '{"name":"@1flowbase/api-client","type":"module"}\n'
  );
  const authSource = path.join(authRoot, 'index.ts');
  fs.writeFileSync(authSource, 'export const loginEntries = true;\n');
  return { appRoot, authSource };
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('dev generation dependency manifest', () => {
  test('AC-001 AC-003 hashes declared workspace source inputs deterministically', () => {
    const { appRoot, authSource } = createFixture();
    const beforeManifest = createDevGenerationDependencyManifest(
      appRoot,
      'development'
    );
    const beforeIdentity = devCacheIdentity(appRoot, 'development');

    expect(beforeManifest.inputs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          path: 'packages/api-client/src/auth/index.ts',
          kind: 'workspace-source'
        })
      ])
    );
    expect(beforeManifest.inputs.map((input) => input.path)).toEqual(
      [...beforeManifest.inputs.map((input) => input.path)].sort()
    );

    fs.writeFileSync(authSource, 'export const loginEntries = false;\n');

    expect(devCacheIdentity(appRoot, 'development')).not.toBe(beforeIdentity);
  });

  test('AC-003 atomically persists a readable manifest for cache diagnosis', () => {
    const { appRoot } = createFixture();
    const cacheDirectory = path.join(appRoot, 'node_modules', '.generation');
    const manifest = createDevGenerationDependencyManifest(
      appRoot,
      'development'
    );

    const manifestPath = persistDevGenerationDependencyManifest(
      cacheDirectory,
      manifest
    );

    expect(manifestPath).toBe(
      path.join(cacheDirectory, '1flowbase-generation-manifest.json')
    );
    expect(JSON.parse(fs.readFileSync(manifestPath, 'utf8'))).toEqual(manifest);
    expect(fs.readdirSync(cacheDirectory)).toEqual([
      '1flowbase-generation-manifest.json'
    ]);
  });
});

describe('dev runtime diagnostics', () => {
  test('AC-004 preserves validation stage, specifier, and original error', () => {
    const failure = createDevRuntimeError(
      'optimizer_contract',
      new SyntaxError("does not provide an export named 'loginEntries'"),
      '@1flowbase/api-client/auth'
    );

    expect(failure).toEqual({
      stage: 'optimizer_contract',
      specifier: '@1flowbase/api-client/auth',
      name: 'SyntaxError',
      message: "does not provide an export named 'loginEntries'"
    });
  });

  test('AC-005 retains the active generation and one recent immutable predecessor', async () => {
    const { appRoot } = createFixture();
    const generationsRoot = path.join(
      appRoot,
      'node_modules',
      '.vite-generations'
    );
    const active = 'a'.repeat(64);
    const recent = 'b'.repeat(64);
    const stale = 'c'.repeat(64);
    for (const generation of [active, recent, stale]) {
      fs.mkdirSync(path.join(generationsRoot, generation), { recursive: true });
    }
    const now = new Date();
    fs.utimesSync(path.join(generationsRoot, active), now, new Date(1));
    fs.utimesSync(path.join(generationsRoot, stale), now, new Date(2));
    fs.utimesSync(path.join(generationsRoot, recent), now, new Date(3));

    await expect(pruneDevGenerationCaches(appRoot, active)).resolves.toEqual([
      stale
    ]);
    expect(fs.existsSync(path.join(generationsRoot, active))).toBe(true);
    expect(fs.existsSync(path.join(generationsRoot, recent))).toBe(true);
  });
});
