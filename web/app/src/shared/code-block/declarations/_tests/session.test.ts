import type { Monaco } from '@monaco-editor/react';
import { expect, test, vi } from 'vitest';
import { createDeclarationSession } from '../session';

const lib = {
  source: 'react',
  content: 'export const value: string;',
  filePath: 'file:///node_modules/react/index.d.ts'
};
function fixture() {
  const dispose = vi.fn();
  const addExtraLib = vi.fn(() => ({ dispose }));
  const monaco = {
    languages: { typescript: { typescriptDefaults: { addExtraLib } } }
  } as unknown as Monaco;
  return { monaco, addExtraLib, dispose };
}

test('incremental registration retains other drafts and refcounts simultaneous login/frontstage editors', () => {
  const { monaco, addExtraLib, dispose } = fixture();
  const first = createDeclarationSession(monaco);
  const login = createDeclarationSession(monaco);
  first.register([lib]);
  first.register([lib]);
  login.register([lib]);
  expect(addExtraLib).toHaveBeenCalledTimes(1);
  first.dispose();
  expect(dispose).not.toHaveBeenCalled();
  login.dispose();
  expect(dispose).toHaveBeenCalledOnce();
});

test('full import completion waits for declarations, shares its flight and registers exactly once', async () => {
  const { monaco, addExtraLib } = fixture();
  const session = createDeclarationSession(monaco);
  let resolve!: (files: (typeof lib)[]) => void;
  const loader = {
    loadForSource: vi.fn(),
    loadAll: vi.fn(
      () =>
        new Promise<(typeof lib)[]>((done) => {
          resolve = done;
        })
    )
  };
  const first = session.loadAll(loader);
  expect(session.loadAll(loader)).toBe(first);
  expect(addExtraLib).not.toHaveBeenCalled();
  resolve([lib]);
  expect(await first).toBe(true);
  expect(addExtraLib).toHaveBeenCalledWith(lib.content, lib.filePath);
  expect(await session.loadAll(loader)).toBe(false);
  expect(loader.loadAll).toHaveBeenCalledOnce();
});

test('late source responses cannot register into a disposed editor', async () => {
  const { monaco, addExtraLib } = fixture();
  const session = createDeclarationSession(monaco);
  const loader = { loadAll: vi.fn(), loadForSource: vi.fn(async () => [lib]) };
  const flight = session.loadSource(loader, `import 'react'`);
  session.dispose();
  await flight;
  expect(addExtraLib).not.toHaveBeenCalled();
});
