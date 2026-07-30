import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { build } from 'vite';

const WEB_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const REPOSITORY_ROOT = resolve(WEB_ROOT, '..');
const DEFAULT_OUTPUT = join(
  REPOSITORY_ROOT,
  'api/plugins/capability-plugins/1flowbase/browser-assets'
);
const MODULE_DIRECTORIES = [
  'ant-design-icons-catalog',
  'native-components',
  'charts',
  'rich-text'
];

export async function buildOfficialBrowserAssets(
  outputDirectory = DEFAULT_OUTPUT
) {
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
      const moduleOutput = join(stagingDirectory, directoryName);
      await build({
        configFile: false,
        logLevel: 'silent',
        root: packageDirectory,
        resolve: {
          alias: {
            '@ant-design/icons': join(
              WEB_ROOT,
              'app/node_modules/@ant-design/icons'
            ),
            echarts: join(WEB_ROOT, 'app/node_modules/echarts'),
            vditor: join(WEB_ROOT, 'app/node_modules/vditor')
          }
        },
        define: { 'process.env.NODE_ENV': JSON.stringify('production') },
        build: {
          cssCodeSplit: false,
          emptyOutDir: true,
          lib: {
            entry: join(packageDirectory, descriptor.entry),
            formats: ['es'],
            name: directoryName
          },
          minify: 'oxc',
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
        const bytes = await readFile(join(moduleOutput, outputName));
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
          sha256: createHash('sha256').update(bytes).digest('hex')
        });
      }
      digestModules.push({
        module_source: descriptor.module_source,
        module_version: descriptor.module_version,
        exports: [...descriptor.exports].sort(),
        type_declarations: typeDeclarations,
        assets
      });
    }

    const digestInput = {
      format: '1flowbase.official-browser-assets/v1',
      modules: digestModules.sort((left, right) =>
        left.module_source.localeCompare(right.module_source)
      )
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

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await buildOfficialBrowserAssets();
}
