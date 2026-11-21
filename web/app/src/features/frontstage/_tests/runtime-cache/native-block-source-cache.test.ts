import { IDBFactory } from 'fake-indexeddb';
import { sha256Text } from '@1flowbase/page-runtime';
import { describe, expect, test } from 'vitest';

import { FrontstageNativeBlockSourceCache } from '../../lib/runtime-cache/native-block-source-cache';

function createValidSource(label: string) {
  const code = `export default function Block() { return ${JSON.stringify(label)}; }`;
  return {
    block_id: 'block-1',
    page_id: 'page-1',
    source_code: code,
    source_sha256: sha256Text(code)
  };
}

describe('FrontstageNativeBlockSourceCache', () => {
  test('persists across cache instances in IndexedDB and isolates by actor and workspace', async () => {
    const indexedDbFactory = new IDBFactory();
    const firstInstance = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-1',
      indexedDbFactory
    });
    const sourceA = createValidSource('overview');
    const identityA = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };

    expect(await firstInstance.put(identityA, sourceA)).toBe(true);

    // Simulate browser page.reload() with a fresh in-memory L1 cache backed by the same IndexedDB
    const reloadedInstance = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-1',
      indexedDbFactory
    });
    expect(await reloadedInstance.get(identityA)).toEqual(sourceA);
    expect(
      await reloadedInstance.get({
        ...identityA,
        actorId: 'actor-b'
      })
    ).toBeUndefined();
    expect(
      await reloadedInstance.get({
        ...identityA,
        workspaceId: 'workspace-2'
      })
    ).toBeUndefined();
  });

  test('rejects unverified or mismatched SHA-256 digests', async () => {
    const cache = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-2',
      indexedDbFactory: new IDBFactory()
    });
    const identity = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };

    expect(
      await cache.put(identity, {
        block_id: 'block-1',
        page_id: 'page-1',
        source_code: 'export default function Block() { return null; }',
        source_sha256: '0'.repeat(64)
      })
    ).toBe(false);
    expect(
      await cache.put(identity, {
        block_id: 'block-1',
        page_id: 'page-1',
        source_code: 'export default function Block() { return null; }',
        source_sha256: null
      })
    ).toBe(false);
    expect(await cache.get(identity)).toBeUndefined();
  });

  test('deleteActor purges persisted entries and blocks stale pre-logout epoch writes', async () => {
    const indexedDbFactory = new IDBFactory();
    const cache = new FrontstageNativeBlockSourceCache({
      databaseName: 'test-block-sources-3',
      indexedDbFactory
    });
    const identity = {
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      requestId: 'request-1'
    };
    const staleEpoch = cache.captureWriteEpoch('actor-a');
    await cache.put(identity, createValidSource('v1'));

    await cache.deleteActor('actor-a');
    expect(await cache.get(identity)).toBeUndefined();

    // Stale write captured before logout must not re-persist
    expect(
      await cache.put(identity, createValidSource('stale-after-logout'), staleEpoch)
    ).toBe(false);
    expect(await cache.get(identity)).toBeUndefined();
  });
});
