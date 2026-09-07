/* global console */
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const expectedAntdVersion = '6.6.2';
const expectedDrawerVersion = '1.4.2';
const patchRelativePath = `patches/@rc-component__drawer@${expectedDrawerVersion}.patch`;

const appManifest = JSON.parse(
  await readFile(resolve(webRoot, 'app/package.json'), 'utf8')
);
const workspace = await readFile(
  resolve(webRoot, 'pnpm-workspace.yaml'),
  'utf8'
);
const lockfile = await readFile(resolve(webRoot, 'pnpm-lock.yaml'), 'utf8');
const patch = await readFile(resolve(webRoot, patchRelativePath), 'utf8');

const installedAntdVersion = appManifest.dependencies?.antd;
if (installedAntdVersion !== expectedAntdVersion) {
  throw new Error(
    `antd must remain exactly ${expectedAntdVersion} while the rc-drawer motion patch is active; found ${String(installedAntdVersion)}. For an upgrade, verify whether react-component/drawer#591 has shipped and remove this receipt if the upstream fix is present.`
  );
}

if (!lockfile.includes(`'@rc-component/drawer@${expectedDrawerVersion}':`)) {
  throw new Error(
    `pnpm-lock.yaml must resolve @rc-component/drawer@${expectedDrawerVersion} while the local motion patch is active.`
  );
}

const registration = `'@rc-component/drawer@${expectedDrawerVersion}': ${patchRelativePath}`;
if (!workspace.includes(registration)) {
  throw new Error(
    `Missing pnpm patchedDependencies registration: ${registration}`
  );
}

for (const marker of [
  '+  const motionRefCache = React.useRef(null);',
  '+    motionRefCache.current = motionRef;',
  '+      ref: mergedWrapperRef,'
]) {
  if (!patch.includes(marker)) {
    throw new Error(
      `rc-drawer motion patch receipt is missing marker: ${marker}`
    );
  }
}

console.log(
  `@rc-component/drawer@${expectedDrawerVersion} motion ref patch registration verified.`
);
