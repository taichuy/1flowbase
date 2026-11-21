import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, test, vi } from 'vitest';
const { prefetch, runtimeImport } = vi.hoisted(() => ({
  prefetch: vi.fn(),
  runtimeImport: vi.fn()
}));
vi.mock('../lib/public-auth-compilation', () => ({
  prefetchPublicAuthSource: prefetch
}));
vi.mock('../components/PublicAuthBlock', () => {
  runtimeImport();
  return { PublicAuthBlock: () => null };
});
import {
  canPrefetchPublicAuth,
  loadPublicAuthBlock,
  prefetchPublicAuthEntry
} from '../lib/public-auth-loading';
beforeEach(() => {
  prefetch.mockReset();
  runtimeImport.mockClear();
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: undefined
  });
});
describe('I2012 public auth loading', () => {
  test('an exact default-source hover/focus does not fetch the custom host or compiler', async () => {
    const source = readFileSync(
      resolve(
        '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
      ),
      'utf8'
    );
    prefetchPublicAuthEntry(source);
    await Promise.resolve();
    expect(runtimeImport).not.toHaveBeenCalled();
    expect(prefetch).not.toHaveBeenCalled();
  });
  test('AC-001 merges runtime imports and keeps only the newest pending intent', async () => {
    expect(loadPublicAuthBlock()).toBe(loadPublicAuthBlock());
    prefetchPublicAuthEntry('old source');
    prefetchPublicAuthEntry('selected source');
    await loadPublicAuthBlock();
    await Promise.resolve();
    await vi.waitFor(() =>
      expect(prefetch).toHaveBeenCalledExactlyOnceWith('selected source')
    );
  });
  test.each([
    { saveData: true },
    { effectiveType: '2g' },
    { effectiveType: 'slow-2g' }
  ])(
    'AC-001 skips speculation under constrained connection %j',
    async (connection) => {
      Object.defineProperty(navigator, 'connection', {
        configurable: true,
        value: connection
      });
      expect(canPrefetchPublicAuth()).toBe(false);
      prefetchPublicAuthEntry('not needed');
      await Promise.resolve();
      expect(prefetch).not.toHaveBeenCalled();
      // Actual selection may still load its runtime on the same connection.
      expect(await loadPublicAuthBlock()).toHaveProperty('PublicAuthBlock');
    }
  );
});
