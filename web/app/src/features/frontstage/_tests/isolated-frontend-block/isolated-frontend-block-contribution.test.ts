import { describe, expect, test, vi } from 'vitest';

import { sha256Text } from '@1flowbase/page-runtime';

import type { FrontstageBlockCatalogEntry } from '../../api/block-catalog';
import {
  ISOLATED_FRONTEND_UI_MOUNT_PERMISSION,
  prepareFrontstageIsolatedContribution,
  type FrontstageIsolatedContributionExpectation
} from '../../lib/isolated-frontend-block-contribution';

const SOURCE = `globalThis.__oneflowbaseIsolatedBlock = {
  mount(root, props) { root.textContent = String(props.title); }
};`;

const EXPECTED: FrontstageIsolatedContributionExpectation = {
  blockInstanceId: 'page-block-1',
  workspaceId: 'workspace-1',
  installationId: 'installation-1',
  providerCode: 'official',
  pluginId: 'official.blocks',
  pluginVersion: '1.0.0',
  contributionCode: 'isolated-chart',
  props: { title: 'Quarterly revenue' }
};

describe('isolated frontend contribution preparation', () => {
  test('D5-P3 prepares only an exact typed independent-Realm binding with verified bytes', async () => {
    const fetchAsset = vi.fn(
      async () =>
        new Response(SOURCE, {
          status: 200,
          headers: { 'content-type': 'text/javascript; charset=utf-8' }
        })
    );

    const prepared = await prepareFrontstageIsolatedContribution(
      createCatalogEntry(),
      EXPECTED,
      fetchAsset
    );

    expect(prepared).toEqual({
      state: 'prepared',
      blockInstanceId: 'page-block-1',
      contributionId: 'frontend-block.installation-1.isolated-chart',
      blockId: 'installation-1:isolated-chart',
      blockVersion: '1.0.0',
      graphFingerprint: 'graph-fingerprint',
      runtimeKind: 'isolated',
      executionKind: 'ui_mount',
      isolationRequirement: 'independent_realm',
      lifecycleKind: 'workspace_assignment',
      grantedPermissions: [ISOLATED_FRONTEND_UI_MOUNT_PERMISSION],
      assetIntegrity: 'verified_sha256',
      program: { source: SOURCE, props: { title: 'Quarterly revenue' } }
    });
    expect(fetchAsset).toHaveBeenCalledWith(
      expect.stringMatching(new RegExp(`${sha256Text(SOURCE)}$`, 'u')),
      {
        credentials: 'same-origin',
        headers: { Accept: 'text/javascript; charset=utf-8' }
      }
    );
  });

  test.each([
    ['trusted runtime', { runtime_kind: 'trusted_native' }],
    ['host Realm', { isolation_requirement: 'trusted_host_realm' }],
    ['missing permission', { granted_permissions: [] }],
    ['disabled assignment', { disable_reason: 'assignment_stale' }]
  ] as const)(
    'rejects %s before fetching an asset',
    async (_label, override) => {
      const fetchAsset = vi.fn();
      await expect(
        prepareFrontstageIsolatedContribution(
          createCatalogEntry(override as Partial<FrontstageBlockCatalogEntry>),
          EXPECTED,
          fetchAsset
        )
      ).rejects.toThrow('Isolated frontend contribution rejected');
      expect(fetchAsset).not.toHaveBeenCalled();
    }
  );

  test('rejects digest mismatches and non-self-contained module source', async () => {
    const entry = createCatalogEntry();
    await expect(
      prepareFrontstageIsolatedContribution(
        entry,
        EXPECTED,
        async () => new Response(`${SOURCE}\n// changed`, { status: 200 })
      )
    ).rejects.toThrow('asset digest mismatch');

    const importedSource = "import('https://example.test/module.js')";
    await expect(
      prepareFrontstageIsolatedContribution(
        createCatalogEntry(importedSource),
        EXPECTED,
        async () => new Response(importedSource, { status: 200 })
      )
    ).rejects.toThrow('without imports or exports');
  });
});

function createCatalogEntry(
  sourceOrOverrides: string | Partial<FrontstageBlockCatalogEntry> = SOURCE
): FrontstageBlockCatalogEntry {
  const source =
    typeof sourceOrOverrides === 'string' ? sourceOrOverrides : SOURCE;
  const overrides =
    typeof sourceOrOverrides === 'string' ? {} : sourceOrOverrides;
  const digest = sha256Text(source);
  return {
    installation_id: 'installation-1',
    provider_code: 'official',
    plugin_id: 'official.blocks',
    plugin_version: '1.0.0',
    contribution_code: 'isolated-chart',
    title: 'Isolated chart',
    runtime: 'isolated_iframe',
    entry: '@1flowbase/isolated-chart',
    code_modules: [
      {
        source: '@1flowbase/isolated-chart',
        version: '1.0.0',
        binding: 'fetched',
        assets: [
          {
            role: 'browser_module',
            media_type: 'text/javascript; charset=utf-8',
            sha256: digest,
            url: `/api/console/frontstage/component-module-assets/${digest}`,
            integrity: 'verified_sha256'
          }
        ],
        exports: [],
        type_declarations: ''
      }
    ],
    context_contract: { primitives: [], input_schema: { type: 'object' } },
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    ui_capabilities: [],
    frontend_contribution_id: 'frontend-block.installation-1.isolated-chart',
    frontend_block_id: 'installation-1:isolated-chart',
    frontend_block_version: '1.0.0',
    runtime_kind: 'isolated',
    execution_kind: 'ui_mount',
    isolation_requirement: 'independent_realm',
    requested_permissions: [ISOLATED_FRONTEND_UI_MOUNT_PERMISSION],
    granted_permissions: [ISOLATED_FRONTEND_UI_MOUNT_PERMISSION],
    workspace_id: 'workspace-1',
    lifecycle_kind: 'workspace_assignment',
    graph_fingerprint: 'graph-fingerprint',
    provenance: {
      module_id: '1flowbase.boot.core',
      module_version: '1',
      module_kind: 'boot_core'
    },
    disable_reason: null,
    ...overrides
  };
}
