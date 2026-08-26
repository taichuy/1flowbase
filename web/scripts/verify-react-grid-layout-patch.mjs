/* global console */
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const expectedVersion = '2.2.3';
const patchRelativePath = `patches/react-grid-layout@${expectedVersion}.patch`;

const appManifest = JSON.parse(
  await readFile(resolve(webRoot, 'app/package.json'), 'utf8')
);
const workspace = await readFile(
  resolve(webRoot, 'pnpm-workspace.yaml'),
  'utf8'
);
const patch = await readFile(resolve(webRoot, patchRelativePath), 'utf8');

const installedVersion = appManifest.dependencies?.['react-grid-layout'];
if (installedVersion !== expectedVersion) {
  throw new Error(
    `react-grid-layout must remain exactly ${expectedVersion} while the local edge-scroll patch is active; found ${String(installedVersion)}. For an upgrade, remove the patch registration and this receipt first, run the issue-1899 browser fixture against the new version, and only migrate the patch if that fixture still fails.`
  );
}

const registration = `react-grid-layout@${expectedVersion}: ${patchRelativePath}`;
if (!workspace.includes(registration)) {
  throw new Error(`Missing pnpm patchedDependencies registration: ${registration}`);
}

for (const marker of [
  'EDGE_SCROLL_MAX_SPEED',
  'scrollDelta / transformScale',
  'scheduleEdgeScroll(mouseEvent, node)'
]) {
  if (!patch.includes(marker)) {
    throw new Error(`react-grid-layout patch receipt is missing marker: ${marker}`);
  }
}

console.log(
  `react-grid-layout@${expectedVersion} edge-scroll patch registration verified.`
);
