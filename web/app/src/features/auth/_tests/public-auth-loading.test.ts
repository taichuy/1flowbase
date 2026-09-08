import { beforeEach, describe, expect, test, vi } from 'vitest';
const { prefetch } = vi.hoisted(() => ({ prefetch: vi.fn() }));
vi.mock('../lib/public-auth-compilation', () => ({
  prefetchPublicAuthSource: prefetch
}));
vi.mock('../components/PublicAuthBlock', () => ({
  PublicAuthBlock: () => null
}));
import {
  canPrefetchPublicAuth,
  loadPublicAuthBlock,
  prefetchPublicAuthEntry
} from '../lib/public-auth-loading';
beforeEach(() => {
  prefetch.mockReset();
  Object.defineProperty(navigator, 'connection', {
    configurable: true,
    value: undefined
  });
});
describe('I2012 public auth loading', () => {
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
