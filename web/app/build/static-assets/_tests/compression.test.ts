import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, readdir, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { brotliCompressSync, brotliDecompressSync, constants, gunzipSync, gzipSync } from 'node:zlib';
import { expect, test } from 'vitest';
import { staticAssetCompressionPlugin } from '../compression';

const brotliOptions = {
  params: {
    [constants.BROTLI_PARAM_QUALITY]: 11,
    [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT
  }
};

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), 'flowbase-compression-'));
  return {
    directory,
    async put(name: string, source: string | Uint8Array) {
      const file = join(directory, name);
      await mkdir(dirname(file), { recursive: true });
      await writeFile(file, source);
      return file;
    },
    async run() {
      const plugin = staticAssetCompressionPlugin();
      const hook = plugin.writeBundle;
      if (typeof hook !== 'object') throw new Error('Expected final ordered hook');
      expect(plugin.generateBundle).toBeUndefined();
      expect(hook.order).toBe('post');
      expect(hook.sequential).toBe(true);
      await (hook.handler as Function).call(
        { error: (message: string) => { throw new Error(message); } },
        { dir: directory },
        {}
      );
    },
    dispose: () => rm(directory, { recursive: true, force: true })
  };
}

async function expectVariants(file: string, bytes: Buffer) {
  expect(await readFile(file)).toEqual(bytes);
  expect(await readFile(`${file}.gz`)).toEqual(gzipSync(bytes, { level: 9 }));
  expect(await readFile(`${file}.br`)).toEqual(brotliCompressSync(bytes, brotliOptions));
  expect(gunzipSync(await readFile(`${file}.gz`))).toEqual(bytes);
  expect(brotliDecompressSync(await readFile(`${file}.br`))).toEqual(bytes);
}

test('511/512 is a byte boundary for binary and multibyte UTF-8 originals', async () => {
  const f = await fixture();
  try {
    const utf8 = '中'.repeat(170);
    const cases = {
      'binary-511.wasm': Buffer.alloc(511, 0xff),
      'binary-512.wasm': Buffer.alloc(512, 0xff),
      'utf8-511.json': Buffer.from(`${utf8}a`),
      'utf8-512.json': Buffer.from(`${utf8}ab`)
    };
    for (const [name, bytes] of Object.entries(cases)) await f.put(name, bytes);
    await f.run();
    for (const [name, bytes] of Object.entries(cases)) {
      const file = join(f.directory, name);
      expect(await readFile(file)).toEqual(bytes);
      if (bytes.length === 512) await expectVariants(file, bytes);
      else {
        await expect(readFile(`${file}.gz`)).rejects.toMatchObject({ code: 'ENOENT' });
        await expect(readFile(`${file}.br`)).rejects.toMatchObject({ code: 'ENOENT' });
      }
    }
  } finally {
    await f.dispose();
  }
});

test('all six final extensions use exact gz9/br11-text bytes and preserve original bytes/mtime', async () => {
  const f = await fixture();
  try {
    const backing = Buffer.concat([Buffer.from('ignore'), Buffer.alloc(2048, 0xfe), Buffer.from('ignore')]);
    const bytes = Buffer.from(backing.subarray(6, -6));
    const files = [];
    for (const extension of ['js', 'css', 'html', 'svg', 'json', 'wasm']) {
      const file = await f.put(`nested/original.${extension}`, backing.subarray(6, -6));
      await utimes(file, 1700000000.125, 1700000000.125);
      files.push(file);
    }
    await f.run();
    const first = new Map<string, Buffer>();
    for (const file of files) {
      await expectVariants(file, bytes);
      const original = await stat(file);
      expect(original.mtimeMs).toBe(1700000000125);
      for (const encoding of ['gz', 'br']) {
        expect((await stat(`${file}.${encoding}`)).mtimeMs).toBeCloseTo(original.mtimeMs, 2);
        first.set(`${file}.${encoding}`, await readFile(`${file}.${encoding}`));
      }
    }
    // A different source mtime must not leak into the gzip header/output bytes.
    for (const file of files) await utimes(file, 1700000001, 1700000001);
    await f.run();
    for (const [sidecar, compressed] of first) {
      expect(await readFile(sidecar)).toEqual(compressed);
      expect((await stat(sidecar)).mtimeMs).toBe(1700000001000);
    }
  } finally {
    await f.dispose();
  }
});

test('omits each nonbeneficial variant independently and removes stale sidecars', async () => {
  const f = await fixture();
  try {
    const entropy = Buffer.concat(Array.from({ length: 16 }, (_, index) =>
      createHash('sha256').update(`compression-fixture-${index}`).digest()
    ));
    let sawBrotliOnly = false;
    const samples = [];
    for (const padding of [0, 8, 16, 24, 32, 40, 48, 56, 64]) {
      const bytes = Buffer.from(entropy);
      bytes.fill(0, 512 - padding);
      const file = await f.put(`entropy-${padding}.wasm`, bytes);
      await f.put(`entropy-${padding}.wasm.gz`, 'stale gzip');
      await f.put(`entropy-${padding}.wasm.br`, 'stale brotli');
      const variants = {
        gz: gzipSync(bytes, { level: 9 }),
        br: brotliCompressSync(bytes, brotliOptions)
      };
      if (padding === 0) {
        expect(variants.gz.length).toBeGreaterThanOrEqual(bytes.length);
        expect(variants.br.length).toBeGreaterThanOrEqual(bytes.length);
      }
      sawBrotliOnly ||= variants.gz.length >= bytes.length && variants.br.length < bytes.length;
      samples.push({ file, bytes, variants });
    }
    await f.run();
    for (const { file, bytes, variants } of samples) {
      expect(await readFile(file)).toEqual(bytes);
      for (const [encoding, compressed] of Object.entries(variants)) {
        if (compressed.length < bytes.length) expect(await readFile(`${file}.${encoding}`)).toEqual(compressed);
        else await expect(readFile(`${file}.${encoding}`)).rejects.toMatchObject({ code: 'ENOENT' });
      }
    }
    expect(sawBrotliOnly).toBe(true);
    await f.put('shrunk.js', 'short');
    await f.put('shrunk.js.gz', 'old gzip');
    await f.put('shrunk.js.br', 'old brotli');
    await f.run();
    await expect(readFile(join(f.directory, 'shrunk.js.gz'))).rejects.toMatchObject({ code: 'ENOENT' });
    await expect(readFile(join(f.directory, 'shrunk.js.br'))).rejects.toMatchObject({ code: 'ENOENT' });
  } finally {
    await f.dispose();
  }
});

test('does not compress unrelated extensions or already compressed sidecars', async () => {
  const f = await fixture();
  try {
    const bytes = Buffer.alloc(1024, 0x61);
    const names = ['image.png', 'font.woff2', 'entry.js.map', 'orphan.js.gz', 'orphan.css.br'];
    for (const name of names) await f.put(name, bytes);
    await f.run();
    expect((await readdir(f.directory)).sort()).toEqual(names.sort());
    for (const name of names) expect(await readFile(join(f.directory, name))).toEqual(bytes);
  } finally {
    await f.dispose();
  }
});
