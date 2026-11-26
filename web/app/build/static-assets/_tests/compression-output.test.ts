import { mkdtemp, mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { brotliCompressSync, brotliDecompressSync, constants, gunzipSync, gzipSync } from 'node:zlib';
import { expect, test } from 'vitest';
import { staticAssetCompressionPlugin } from '../compression';

// A tiny real Vite output fixture, not the application build. In particular,
// publicDir files never appear in generateBundle's bundle object.
test('compresses final Vite/publicDir bytes after post HTML/manifest emission and async disk writers', async () => {
  const { build } = await import('vite');
  const root = await mkdtemp(join(tmpdir(), 'flowbase-compression-output-'));
  const output = join(root, 'dist');
  const put = async (name: string, source: string | Uint8Array) => {
    const file = join(root, name);
    await mkdir(dirname(file), { recursive: true });
    await writeFile(file, source);
  };
  const html = `<!doctype html><main>${'最终文档'.repeat(150)}</main>`;
  const manifest = JSON.stringify({ final: true, files: ['late.js'.repeat(150)] });
  const diskBytes = Buffer.from(JSON.stringify({ disk: 'final'.repeat(200) }));
  try {
    await put('index.html', '<!doctype html><html><body><script type="module" src="/entry.js"></script></body></html>');
    await put('entry.js', `import './entry.css'; document.body.dataset.fixture = ${JSON.stringify('entry'.repeat(200))};`);
    await put('entry.css', Array.from({ length: 30 }, (_, i) => `.fixture-${i} { color: red; }`).join('\n'));
    await put('public/nested/public.svg', `<svg><!--${'public'.repeat(200)}--></svg>`);
    await put('public/nested/public.wasm', Buffer.alloc(1024, 0xfe));
    await put('public/nested/public.json', manifest);
    await put('public/nested/public.json.gz', 'stale public gzip');
    await put('public/nested/public.json.br', 'stale public brotli');
    await build({
      configFile: false,
      root,
      logLevel: 'silent',
      plugins: [
        staticAssetCompressionPlugin(),
        {
          name: 'late-output-fixture',
          enforce: 'post',
          transformIndexHtml: {
            order: 'post',
            handler: (source) => source.replace('</body>', `<p>${'late HTML'.repeat(100)}</p></body>`)
          },
          generateBundle: {
            order: 'post',
            handler(_options, bundle) {
              for (const file of Object.values(bundle)) {
                if (file.type === 'chunk') file.code += '\n// final post-generated code\n';
              }
              this.emitFile({ type: 'asset', fileName: 'sign-in-preload.html', source: html });
              this.emitFile({ type: 'asset', fileName: '.vite/late-manifest.json', source: manifest });
            }
          },
          // This ordinary async writer is registered AFTER compression. A post,
          // sequential writeBundle must nevertheless wait for its final bytes.
          async writeBundle() {
            await writeFile(join(output, 'late-disk.json'), diskBytes);
          }
        }
      ],
      build: {
        outDir: output,
        emptyOutDir: true,
        minify: false,
        cssMinify: false,
        manifest: true
      }
    });
    expect(await readFile(join(output, 'sign-in-preload.html'), 'utf8')).toBe(html);
    expect(await readFile(join(output, '.vite/late-manifest.json'), 'utf8')).toBe(manifest);
    expect(await readFile(join(output, 'late-disk.json'))).toEqual(diskBytes);
    expect(await readFile(join(root, 'public/nested/public.json.gz'), 'utf8')).toBe('stale public gzip');
    expect(await readFile(join(output, 'index.html'), 'utf8')).toContain('late HTML');
    expect(JSON.parse(await readFile(join(output, '.vite/manifest.json'), 'utf8'))).toHaveProperty('index.html');
    const files = await readdir(output, { recursive: true });
    const originals = files.filter((name) => /\.(?:js|css|html|svg|json|wasm)$/u.test(name));
    expect(originals).toEqual(expect.arrayContaining([
      'index.html', 'sign-in-preload.html', '.vite/late-manifest.json',
      '.vite/manifest.json', 'late-disk.json', 'nested/public.svg',
      'nested/public.json', 'nested/public.wasm'
    ]));
    expect(originals.some((name) => name.endsWith('.css'))).toBe(true);
    const script = originals.find((name) => name.endsWith('.js'));
    expect(script).toBeDefined();
    expect(await readFile(join(output, script!), 'utf8')).toContain('final post-generated code');
    for (const name of originals) {
      const file = join(output, name);
      const bytes = await readFile(file);
      const variants = {
        gz: gzipSync(bytes, { level: 9 }),
        br: brotliCompressSync(bytes, {
          params: {
            [constants.BROTLI_PARAM_QUALITY]: 11,
            [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT
          }
        })
      };
      for (const [encoding, compressed] of Object.entries(variants)) {
        const sidecar = `${file}.${encoding}`;
        if (bytes.length >= 512 && compressed.length < bytes.length) {
          expect(await readFile(sidecar)).toEqual(compressed);
          const decode = encoding === 'gz' ? gunzipSync : brotliDecompressSync;
          expect(decode(await readFile(sidecar))).toEqual(bytes);
          expect((await stat(sidecar)).mtimeMs).toBeCloseTo((await stat(file)).mtimeMs, 2);
        } else {
          await expect(readFile(sidecar)).rejects.toMatchObject({ code: 'ENOENT' });
        }
      }
    }
    expect(files.some((name) => /\.(?:gz|br)\.(?:gz|br)$/u.test(name))).toBe(false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
