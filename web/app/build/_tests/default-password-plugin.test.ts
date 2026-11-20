import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { transformWithOxc } from 'vite';
import { expect, test, vi } from 'vitest';
import { publicSignInPlugin } from '../public-sign-in';

const root = process.cwd();
const template = path.resolve(
  root,
  '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
);
const source = readFileSync(template, 'utf8');
const fastId = 'virtual:1flowbase-default-password';

test('compiles the original backend source as conventional React, changing only import resolution', async () => {
  const plugin = publicSignInPlugin(root);
  const load = plugin.load;
  if (typeof load !== 'function') throw new Error('Expected a load hook');
  const addWatchFile = vi.fn();
  const result = await load.call({ addWatchFile } as never, '\0' + fastId);
  const compiled = await transformWithOxc(source, template, {
    lang: 'tsx',
    jsx: { runtime: 'automatic' },
    sourcemap: true
  });
  if (!result || typeof result === 'string')
    throw new Error('Expected conventional React code');
  expect(result.code).toContain(compiled.code);
  expect(result.code).toContain(
    createHash('sha256').update(source).digest('hex')
  );
  expect(result.code).not.toMatch(
    /eval\(|new Function|artifact|compileNative|PublicAuthBlock/
  );
  expect(addWatchFile).toHaveBeenCalledWith(template);
});

test('primitive aliases apply only to the official compiled module; custom/legacy modules keep real Antd', () => {
  const plugin = publicSignInPlugin(root);
  const resolveId = plugin.resolveId;
  if (typeof resolveId !== 'function')
    throw new Error('Expected a resolve hook');
  const resolve = (id: string, importer?: string) =>
    resolveId.call({} as never, id, importer, {} as never);
  expect(resolve(fastId)).toBe('\0' + fastId);
  for (const name of ['antd', '@ant-design/icons']) {
    expect(resolve(name, '\0' + fastId)).toBe(
      path.resolve(
        root,
        'src/features/auth/components/default-password/primitives.tsx'
      )
    );
    expect(resolve(name, '/custom/provider.tsx')).toBeUndefined();
    expect(
      resolve(name, '\0virtual:1flowbase-default-login-artifact')
    ).toBeUndefined();
  }
  expect(resolve('react', '\0' + fastId)).toBeUndefined();
});

test('keeps the existing compiled artifact module and exact source SHA', async () => {
  const plugin = publicSignInPlugin(root);
  const load = plugin.load;
  if (typeof load !== 'function') throw new Error('Expected a load hook');
  const result = await load.call(
    { addWatchFile: vi.fn(), warn: vi.fn() } as never,
    '\0virtual:1flowbase-default-login-artifact'
  );
  expect(result).toMatch(/^export default /);
  expect(result).toContain(createHash('sha256').update(source).digest('hex'));
});
