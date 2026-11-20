import { readdir, readFile, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { promisify } from 'node:util';
import { brotliCompress, constants, gzip } from 'node:zlib';
import type { Plugin } from 'vite';

const compressGzip = promisify(gzip);
const compressBrotli = promisify(brotliCompress);
const compressibleFile = /\.(?:js|mjs|css|html|svg|json|wasm)$/u;

async function compressDirectory(directory: string): Promise<void> {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  for (const entry of entries) {
    const file = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await compressDirectory(file);
      continue;
    }
    // Do not follow symlinks outside the output or recompress .gz/.br sidecars.
    if (!entry.isFile() || !compressibleFile.test(entry.name)) continue;
    const original = await stat(file);
    const bytes = original.size >= 512 ? await readFile(file) : undefined;
    // One file and one encoding at a time; use Node's existing zlib pool.
    for (const encoding of ['gz', 'br'] as const) {
      const sidecar = `${file}.${encoding}`;
      const compressed = bytes
        ? encoding === 'gz'
          ? await compressGzip(bytes, { level: 9 })
          : await compressBrotli(bytes, {
              params: {
                [constants.BROTLI_PARAM_QUALITY]: 11,
                [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT
              }
            })
        : undefined;
      if (!bytes || !compressed || compressed.length >= bytes.length) {
        // A copied public sidecar or a previous watch output must not survive
        // when its final original is too short or no longer benefits.
        await rm(sidecar, { force: true });
        continue;
      }
      await writeFile(sidecar, compressed);
      // gzip_static uses the sidecar's mtime for validators, not the original's.
      await utimes(sidecar, original.atimeMs / 1000, original.mtimeMs / 1000);
    }
  }
}

/**
 * Compress final disk bytes, including publicDir copies and late HTML/manifests.
 * Register after other post writeBundle writers; no writer may change originals
 * afterwards (including closeBundle). Sidecars are not part of the module graph.
 * The host decides which encodings it can serve; this does not enable Brotli.
 */
export function staticAssetCompressionPlugin(): Plugin {
  return {
    name: '1flowbase-static-asset-compression',
    apply: 'build',
    enforce: 'post',
    writeBundle: {
      order: 'post',
      sequential: true,
      async handler(options) {
        if (!options.dir) {
          this.error('Static asset compression requires an output directory.');
        }
        await compressDirectory(resolve(options.dir));
      }
    }
  };
}
