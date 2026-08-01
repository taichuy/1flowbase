import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  applyConsoleInstalledMcpExtension,
  previewConsoleInstalledMcpExtension
} from '../console/extensions';

import {
  clearConsoleHostInfrastructureCacheDomain,
  clearConsoleHostInfrastructureCacheEntry,
  getConsoleHostInfrastructureMemoryOverview,
  getConsoleHostInfrastructureMemoryStatsOverview,
  getConsoleHostInfrastructureMemoryStats,
  installConsolePluginCurrentNodeArtifact,
  getConsoleHostInfrastructureCacheOverview,
  listConsoleHostInfrastructureCacheEntries,
  listConsoleHostInfrastructureMemoryEntries,
  listConsoleHostInfrastructureMemoryTree,
  refreshConsolePluginCurrentNodeArtifact,
  searchConsoleHostInfrastructureMemoryEntries,
  revealConsoleHostInfrastructureMemoryEntry,
  revealConsoleHostInfrastructureCacheEntry
} from '../console-plugins';

describe('console-plugins current node artifact client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('refreshes current node artifact with csrf', async () => {
    await expect(
      refreshConsolePluginCurrentNodeArtifact('installation-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/plugins/installation-1/artifact/refresh',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('installs current node artifact with csrf', async () => {
    await expect(
      installConsolePluginCurrentNodeArtifact('installation-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/plugins/installation-1/artifact/install-current-node',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });
});

describe('console extension MCP workspace application client', () => {
  test('previews an installed artifact through the MCP settings scope route', async () => {
    await expect(
      previewConsoleInstalledMcpExtension('installation-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/mcp/bundles/preview-official',
      method: 'POST',
      body: { extension_installation_id: 'installation-1' },
      csrfToken: 'csrf-123'
    });
  });

  test('applies with an explicit keep-existing conflict resolution', async () => {
    await expect(
      applyConsoleInstalledMcpExtension(
        'installation-1',
        'csrf-123',
        'keep_existing'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/bundles/import-official',
      method: 'POST',
      body: {
        extension_installation_id: 'installation-1',
        conflict_resolution: 'keep_existing'
      },
      csrfToken: 'csrf-123'
    });
  });
});

describe('console-plugins host infrastructure cache client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('points cache overview at the host infrastructure cache route', async () => {
    await expect(
      getConsoleHostInfrastructureCacheOverview()
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/cache'
    });
  });

  test('points cache entries at the selected domain route', async () => {
    await expect(
      listConsoleHostInfrastructureCacheEntries('application-logs')
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/cache/domains/application-logs/entries'
    });
  });

  test('reveals cache entry value with csrf', async () => {
    await expect(
      revealConsoleHostInfrastructureCacheEntry(
        'application-logs',
        'application-logs:run:1',
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/cache/domains/application-logs/entries/reveal',
      method: 'POST',
      body: { key: 'application-logs:run:1' },
      csrfToken: 'csrf-123'
    });
  });

  test('clears cache entry with csrf', async () => {
    await expect(
      clearConsoleHostInfrastructureCacheEntry(
        'application-logs',
        'application-logs:run:1',
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/cache/domains/application-logs/entries/clear',
      method: 'POST',
      body: { key: 'application-logs:run:1' },
      csrfToken: 'csrf-123'
    });
  });

  test('clears cache domain with csrf', async () => {
    await expect(
      clearConsoleHostInfrastructureCacheDomain('application-logs', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/cache/domains/application-logs/clear',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });
});

describe('console-plugins host infrastructure memory client', () => {
  test('points memory overview at the host infrastructure memory route', async () => {
    await expect(
      getConsoleHostInfrastructureMemoryOverview()
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory'
    });
  });

  test('points memory stats overview at the dedicated stats route', async () => {
    await expect(
      getConsoleHostInfrastructureMemoryStatsOverview()
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/stats'
    });
  });

  test('points memory stats at the dedicated contract stats route', async () => {
    await expect(
      getConsoleHostInfrastructureMemoryStats('session-store', {
        inspection_path: ['workspace-1', 'user-1']
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/contracts/session-store/stats?path=workspace-1%2Fuser-1'
    });
  });

  test('points memory entries at the selected contract route with cursor params', async () => {
    await expect(
      listConsoleHostInfrastructureMemoryEntries('session-store', {
        inspection_path: ['workspace-1', 'user-1'],
        cursor: 'cursor-1',
        limit: 25,
        byte_limit: 4096
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/contracts/session-store/entries?path=workspace-1%2Fuser-1&cursor=cursor-1&limit=25&byte_limit=4096'
    });
  });

  test('points memory tree and search at paged inspection routes', async () => {
    await expect(
      listConsoleHostInfrastructureMemoryTree('cache-store', {
        inspection_path: ['application-logs'],
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/contracts/cache-store/tree?path=application-logs&limit=10'
    });
    await expect(
      searchConsoleHostInfrastructureMemoryEntries('cache-store', {
        q: 'run:2',
        inspection_path: ['application-logs'],
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/search?path=application-logs&limit=10&q=run%3A2'
    });
  });

  test('reveals memory entry value with csrf', async () => {
    await expect(
      revealConsoleHostInfrastructureMemoryEntry(
        'session-store',
        'session:1',
        'csrf-123',
        'full'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/host-infrastructure/memory/contracts/session-store/entries/reveal',
      method: 'POST',
      body: { entry_ref: 'session:1', reveal_mode: 'full' },
      csrfToken: 'csrf-123'
    });
  });
});
