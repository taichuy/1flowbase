import { afterEach, beforeEach, expect, test, vi } from 'vitest';

import {
  recoverDevModuleGraph,
  resetDevModuleRecovery
} from '../dev-module-recovery';

const reload = vi.fn();
const failedImport = new TypeError(
  'Failed to fetch dynamically imported module: /src/settings.tsx'
);

beforeEach(() => {
  vi.stubEnv('DEV', true);
  vi.stubGlobal('window', { location: { reload } });
  sessionStorage.clear();
  document.head.innerHTML =
    '<meta name="1flowbase-dev-generation" content="generation-a">';
  reload.mockClear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  document.head.innerHTML = '';
});

test('recovers a rejected lazy import once across module reloads and boundaries', async () => {
  expect(recoverDevModuleGraph(failedImport)).toBe(true);
  vi.resetModules();
  const afterReload = await import('../dev-module-recovery');
  expect(afterReload.recoverDevModuleGraph(failedImport)).toBe(false);
  expect(reload).toHaveBeenCalledTimes(1);
});

test.each([
  'Importing a module script failed.',
  'Outdated Optimize Dep',
  "The requested module does not provide an export named 'default'"
])('recovers module graph failure: %s', (message) => {
  expect(recoverDevModuleGraph(new Error(message))).toBe(true);
  expect(reload).toHaveBeenCalledOnce();
});

test.each([
  'Failed to fetch',
  'Permission denied',
  'Cannot read properties of undefined'
])('leaves unrelated errors to their existing boundary: %s', (message) => {
  expect(recoverDevModuleGraph(new Error(message))).toBe(false);
  expect(reload).not.toHaveBeenCalled();
});

test('does not automatically reload production', () => {
  vi.stubEnv('DEV', false);
  expect(recoverDevModuleGraph(failedImport)).toBe(false);
  expect(reload).not.toHaveBeenCalled();
});

test('allows a new attempt after a subsequent HMR update', () => {
  recoverDevModuleGraph(failedImport);
  resetDevModuleRecovery();
  expect(recoverDevModuleGraph(failedImport)).toBe(true);
  expect(recoverDevModuleGraph(failedImport)).toBe(false);
  expect(reload).toHaveBeenCalledTimes(2);
});

test('allows a new server generation to recover independently', () => {
  recoverDevModuleGraph(failedImport);
  document.querySelector('meta')!.setAttribute('content', 'generation-b');
  expect(recoverDevModuleGraph(failedImport)).toBe(true);
  expect(reload).toHaveBeenCalledTimes(2);
});

test('cannot loop if browser storage is unavailable', () => {
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new DOMException('Storage disabled', 'SecurityError');
  });
  expect(recoverDevModuleGraph(failedImport)).toBe(false);
  expect(reload).not.toHaveBeenCalled();
});
