import { spawn, type ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import lockfile from 'proper-lockfile';
import { createServer } from 'vite';
import { afterEach, expect, test } from 'vitest';

import {
  pruneDevGenerationCaches,
  retainDevGenerationCache
} from '../dev-generation-cache';
import {
  devGenerationCacheDirectory,
  oneFlowbaseDevRuntimePlugin
} from '../dev-runtime';

const directories: string[] = [];
const children: ChildProcess[] = [];

function fixture() {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), '1flowbase-cache-owners-')
  );
  directories.push(root);
  const cacheRoot = path.join(root, 'node_modules', '.vite-generations');
  const generations = ['a', 'b', 'c'].map((letter) => letter.repeat(64));
  for (const [index, generation] of generations.entries()) {
    const directory = path.join(cacheRoot, generation);
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(path.join(directory, 'Icon.js'), 'export default "icon";');
    fs.utimesSync(directory, new Date(index + 1), new Date(index + 1));
  }
  return { root, cacheRoot, live: generations[0]!, incoming: generations[2]! };
}

function startOwner(root: string, generation: string) {
  const moduleUrl = pathToFileURL(
    path.resolve('vite/dev-generation-cache.ts')
  ).href;
  const child = spawn(
    process.execPath,
    [
      '--experimental-strip-types',
      '--input-type=module',
      '-e',
      `
    const { retainDevGenerationCache } = await import(${JSON.stringify(moduleUrl)});
    const release = await retainDevGenerationCache(${JSON.stringify(root)}, ${JSON.stringify(generation)});
    process.send('retained');
    process.on('message', async () => {
      await release();
      process.disconnect();
    });
  `
    ],
    { stdio: ['ignore', 'ignore', 'pipe', 'ipc'] }
  );
  children.push(child);
  let stderr = '';
  child.stderr?.on('data', (chunk) => {
    stderr += String(chunk);
  });
  const ready = Promise.race([
    once(child, 'message'),
    once(child, 'exit').then(([code]) => {
      throw new Error(`Cache owner exited (${code}): ${stderr.slice(0, 2000)}`);
    })
  ]);
  return { child, ready };
}

afterEach(async () => {
  for (const child of children.splice(0)) {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill('SIGKILL');
      await exited;
    }
  }
  for (const directory of directories.splice(0))
    fs.rmSync(directory, { recursive: true, force: true });
});

test('keeps a shared generation until its last owner releases it', async () => {
  const { root, cacheRoot, live, incoming } = fixture();
  const releaseFirst = await retainDevGenerationCache(root, live);
  const releaseSecond = await retainDevGenerationCache(root, live);
  await releaseFirst();
  expect(await pruneDevGenerationCaches(root, incoming)).not.toContain(live);
  expect(fs.existsSync(path.join(cacheRoot, live, 'Icon.js'))).toBe(true);
  await releaseSecond();
  expect(await pruneDevGenerationCaches(root, incoming)).toContain(live);
  expect(fs.existsSync(path.join(cacheRoot, live))).toBe(false);
});

test('protects a real live child and reclaims its cache after SIGKILL', async () => {
  const { root, cacheRoot, live, incoming } = fixture();
  const { child, ready } = startOwner(root, live);
  expect((await ready)[0]).toBe('retained');
  expect(await pruneDevGenerationCaches(root, incoming)).not.toContain(live);
  expect(fs.existsSync(path.join(cacheRoot, live, 'Icon.js'))).toBe(true);
  const exited = once(child, 'exit');
  child.kill('SIGKILL');
  await exited;
  expect(await pruneDevGenerationCaches(root, incoming)).toContain(live);
  expect(fs.existsSync(path.join(cacheRoot, '.owners', live))).toBe(false);
});

test('registration waits for a collector holding the cross-process lock', async () => {
  const { root, cacheRoot, live } = fixture();
  const unlock = await lockfile.lock(cacheRoot);
  const { child, ready: registered } = startOwner(root, live);
  let completed = false;
  void registered.then(
    () => {
      completed = true;
    },
    () => {}
  );
  try {
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(completed).toBe(false);
    expect(fs.existsSync(path.join(cacheRoot, '.owners', live))).toBe(false);
  } finally {
    await unlock();
  }
  await registered;
  const exited = once(child, 'exit');
  child.send('release');
  await exited;
  expect(fs.readdirSync(path.join(cacheRoot, '.owners', live))).toEqual([]);
});

test('concurrent collectors preserve every occupied generation', async () => {
  const { root, cacheRoot, live, incoming } = fixture();
  const release = await retainDevGenerationCache(root, live);
  try {
    await Promise.all([
      pruneDevGenerationCaches(root, incoming),
      pruneDevGenerationCaches(root, incoming)
    ]);
    expect(fs.readFileSync(path.join(cacheRoot, live, 'Icon.js'), 'utf8')).toBe(
      'export default "icon";'
    );
  } finally {
    await release();
  }
});

test('Vite registers before serving and releases ownership on server.close', async () => {
  const { root } = fixture();
  const cacheDir = devGenerationCacheDirectory(root, 'development');
  const server = await createServer({
    configFile: false,
    root,
    cacheDir,
    server: { middlewareMode: true, watch: null },
    plugins: [
      oneFlowbaseDevRuntimePlugin({
        root,
        mode: 'development',
        command: 'serve'
      })
    ]
  });
  const owners = path.join(
    path.dirname(cacheDir),
    '.owners',
    path.basename(cacheDir)
  );
  try {
    expect(fs.readdirSync(owners)).toHaveLength(1);
  } finally {
    await server.close();
  }
  expect(fs.readdirSync(owners)).toEqual([]);
});

test('custom Vite caches do not register ownership in the managed generations', async () => {
  const { root, cacheRoot } = fixture();
  const server = await createServer({
    configFile: false,
    root,
    cacheDir: path.join(root, '.custom-cache'),
    server: { middlewareMode: true, watch: null },
    plugins: [
      oneFlowbaseDevRuntimePlugin({
        root,
        mode: 'development',
        command: 'serve'
      })
    ]
  });
  try {
    expect(fs.existsSync(path.join(cacheRoot, '.owners'))).toBe(false);
  } finally {
    await server.close();
  }
});
