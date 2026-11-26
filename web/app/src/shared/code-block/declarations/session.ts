import type { Monaco } from '@monaco-editor/react';
import type {
  BlockSourceDeclarationLoader,
  BlockSourceExtraLib
} from '../extra-lib';

type Registration = { content: string; count: number; dispose: () => void };
const registrations = new WeakMap<Monaco, Map<string, Registration>>();

// Monaco defaults are global, while editors (frontstage and custom login) can
// coexist. Unmounting one consumer must not dispose declarations of the other.
export function createDeclarationSession(monaco: Monaco) {
  let shared = registrations.get(monaco);
  if (!shared) {
    shared = new Map();
    registrations.set(monaco, shared);
  }
  const owned = new Map<string, Registration>();
  let disposed = false;
  let completeFlight: Promise<boolean> | undefined;
  let complete = false;
  const release = (filePath: string, entry: Registration) => {
    entry.count -= 1;
    if (entry.count === 0) {
      entry.dispose();
      if (shared.get(filePath) === entry) shared.delete(filePath);
    }
  };
  const register = (files: readonly BlockSourceExtraLib[]) => {
    if (disposed) return;
    for (const file of files) {
      const current = owned.get(file.filePath);
      if (current?.content === file.content) continue;
      if (current) release(file.filePath, current);
      let entry = shared.get(file.filePath);
      if (!entry || entry.content !== file.content) {
        const disposable =
          monaco.languages.typescript.typescriptDefaults.addExtraLib(
            file.content,
            file.filePath
          );
        entry = {
          content: file.content,
          count: 0,
          dispose: () => disposable.dispose()
        };
        shared.set(file.filePath, entry);
      }
      entry.count += 1;
      owned.set(file.filePath, entry);
    }
  };
  return {
    register,
    async loadSource(loader: BlockSourceDeclarationLoader, source: string) {
      register(await loader.loadForSource(source));
    },
    loadAll(loader: BlockSourceDeclarationLoader): Promise<boolean> {
      if (complete || disposed) return Promise.resolve(false);
      if (!completeFlight) {
        completeFlight = loader
          .loadAll()
          .then((files) => {
            register(files);
            complete = true;
            return !disposed;
          })
          .catch((error: unknown) => {
            completeFlight = undefined;
            throw error;
          });
      }
      return completeFlight;
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const [filePath, entry] of owned) release(filePath, entry);
      owned.clear();
    }
  };
}
