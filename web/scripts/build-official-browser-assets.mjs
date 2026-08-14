import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import tailwindcss from '@tailwindcss/vite';
import { build } from 'vite';

import { compileTailwindBlockPreset } from '../packages/tailwindcss-catalog/src/compiler.ts';
import { TAILWIND_BLOCK_PRESET_ASSET } from '../packages/tailwindcss-catalog/src/executable-contract.ts';

const WEB_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const REPOSITORY_ROOT = resolve(WEB_ROOT, '..');
const HOST_ICONS_DIRECTORY = join(
  WEB_ROOT,
  'app/node_modules/@ant-design/icons'
);
const DEFAULT_OUTPUT = join(
  REPOSITORY_ROOT,
  'api/plugins/capability-plugins/1flowbase/browser-assets'
);
const LEGACY_TAILWIND_ASSET_NAME = 'tailwindcss-inventory-v1.css';
const LEGACY_TAILWIND_ASSET_PATH = join(
  DEFAULT_OUTPUT,
  LEGACY_TAILWIND_ASSET_NAME
);
const MODULE_DIRECTORIES = [
  'ant-design-icons-catalog',
  'native-components',
  'charts',
  'rich-text',
  'tailwindcss-catalog'
];

export async function buildOfficialBrowserAssets(
  outputDirectory = DEFAULT_OUTPUT
) {
  const legacyTailwindBytes = await readFile(LEGACY_TAILWIND_ASSET_PATH).catch(
    () =>
      readFile(join(DEFAULT_OUTPUT, 'tailwindcss-catalog.css'))
  );
  const stagingDirectory = join(WEB_ROOT, '.official-browser-assets-staging');
  await rm(stagingDirectory, { force: true, recursive: true });
  await mkdir(stagingDirectory, { recursive: true });

  const digestModules = [];
  try {
    for (const directoryName of MODULE_DIRECTORIES) {
      const packageDirectory = join(WEB_ROOT, 'packages', directoryName);
      const descriptor = JSON.parse(
        await readFile(join(packageDirectory, 'catalog-module.json'), 'utf8')
      );
      const moduleVersion =
        directoryName === 'ant-design-icons-catalog'
          ? JSON.parse(
              await readFile(join(HOST_ICONS_DIRECTORY, 'package.json'), 'utf8')
            ).version
          : descriptor.module_version;
      const moduleOutput = join(stagingDirectory, directoryName);
      let moduleEntry = join(packageDirectory, descriptor.entry);
      if (directoryName === 'tailwindcss-catalog') {
        const preset = await compileTailwindBlockPreset();
        const presetPath = join(
          stagingDirectory,
          TAILWIND_BLOCK_PRESET_ASSET.path
        );
        const entryPath = join(stagingDirectory, 'tailwindcss-catalog-entry.js');
        await writeFile(presetPath, preset.css, 'utf8');
        await writeFile(
          entryPath,
          `import ${JSON.stringify(presetPath)};\nexport { default } from ${JSON.stringify(join(packageDirectory, descriptor.entry))};\n`,
          'utf8'
        );
        moduleEntry = entryPath;
      }
      await build({
        configFile: false,
        logLevel: 'silent',
        root: packageDirectory,
        plugins: [tailwindcss()],
        resolve: {
          alias: {
            '@ant-design/icons': HOST_ICONS_DIRECTORY,
            echarts: join(WEB_ROOT, 'app/node_modules/echarts'),
            vditor: join(WEB_ROOT, 'app/node_modules/vditor')
          }
        },
        define: {
          'process.env.NODE_ENV': JSON.stringify('production'),
          ...(directoryName === 'rich-text'
            ? { define: 'undefined', require: 'undefined' }
            : {})
        },
        build: {
          cssCodeSplit: false,
          emptyOutDir: true,
          lib: {
            entry: moduleEntry,
            formats: ['es'],
            name: directoryName
          },
          // OXC can rename React host imports to identifiers already used by
          // the bundled ECharts runtime, corrupting live EChart instances.
          minify:
            directoryName === 'ant-design-icons-catalog' ||
            directoryName === 'charts'
              ? false
              : 'oxc',
          outDir: moduleOutput,
          sourcemap: false,
          target: 'es2022',
          rollupOptions: {
            external: ['react', 'react/jsx-runtime'],
            output: {
              assetFileNames: `${directoryName}.[ext]`,
              entryFileNames: `${directoryName}.js`,
              exports: 'named',
              format: 'es'
            }
          }
        }
      });

      const outputNames = (await readdir(moduleOutput)).sort();
      const browserAssetName = `${directoryName}.js`;
      if (!outputNames.includes(browserAssetName)) {
        throw new Error(
          `Missing deterministic browser asset: ${browserAssetName}`
        );
      }
      const typeDeclarations = await readFile(
        join(packageDirectory, descriptor.type_declarations),
        'utf8'
      );
      const assets = [];
      for (const outputName of outputNames) {
        const bytes = normalizeOfficialAssetBytes(
          outputName,
          await readFile(join(moduleOutput, outputName))
        );
        const target = join(outputDirectory, outputName);
        await mkdir(dirname(target), { recursive: true });
        await writeFile(target, bytes);
        assets.push({
          path: outputName,
          role:
            outputName === browserAssetName ? 'browser_module' : 'shadow_style',
          media_type: outputName.endsWith('.css')
            ? 'text/css; charset=utf-8'
            : 'text/javascript; charset=utf-8',
          sha256: createHash('sha256').update(bytes).digest('hex'),
          bytes: bytes.byteLength
        });
      }
      const moduleDigestInput = {
        module_source: descriptor.module_source,
        module_version: moduleVersion,
        exports: [...descriptor.exports].sort(),
        type_declarations: typeDeclarations,
        assets,
        ...(descriptor.compiler_identity
          ? { compiler_identity: descriptor.compiler_identity }
          : {}),
        ...(descriptor.toolchain_lock
          ? { toolchain_lock: descriptor.toolchain_lock }
          : {})
      };
      if (directoryName === 'tailwindcss-catalog') {
        const presetAsset = assets.find(
          (asset) => asset.path === TAILWIND_BLOCK_PRESET_ASSET.path
        );
        if (
          !presetAsset ||
          presetAsset.role !== TAILWIND_BLOCK_PRESET_ASSET.role ||
          presetAsset.media_type !== TAILWIND_BLOCK_PRESET_ASSET.media_type ||
          presetAsset.sha256 !== TAILWIND_BLOCK_PRESET_ASSET.sha256
        ) {
          throw new Error(
            'Tailwind block preset asset does not match the executable contract.'
          );
        }
      }
      digestModules.push({
        ...moduleDigestInput,
        content_sha256: sha256Bytes(
          Buffer.from(JSON.stringify(moduleDigestInput))
        )
      });
    }

    await mkdir(outputDirectory, { recursive: true });
    await writeFile(
      join(outputDirectory, LEGACY_TAILWIND_ASSET_NAME),
      legacyTailwindBytes
    );
    const legacyTailwindDigest = sha256Bytes(legacyTailwindBytes);
    const digestInput = {
      format: '1flowbase.official-browser-assets/v2',
      modules: digestModules.sort((left, right) =>
        left.module_source.localeCompare(right.module_source)
      ),
      retained_legacy_assets: [
        {
          identity: 'tailwindcss-inventory-v1',
          path: LEGACY_TAILWIND_ASSET_NAME,
          media_type: 'text/css; charset=utf-8',
          sha256: legacyTailwindDigest,
          bytes: legacyTailwindBytes.byteLength,
          use: 'legacy-recognition-only'
        }
      ]
    };
    await writeFile(
      join(outputDirectory, 'official-browser-assets.digest-input.json'),
      `${JSON.stringify(digestInput, null, 2)}\n`,
      'utf8'
    );
    return digestInput;
  } finally {
    await rm(stagingDirectory, { force: true, recursive: true });
  }
}

function sha256Bytes(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function normalizeOfficialAssetBytes(outputName, bytes) {
  if (!outputName.endsWith('.js')) return bytes;
  return Buffer.from(
    bytes
      .toString('utf8')
      .replace(/^\/\/#region .*\r?\n/gmu, '')
      .replace(/^\/\/#endregion\r?\n/gmu, '')
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await buildOfficialBrowserAssets();
}
