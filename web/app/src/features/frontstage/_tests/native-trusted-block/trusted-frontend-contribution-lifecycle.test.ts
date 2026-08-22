import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, test, vi } from 'vitest';

import type { FrontstageBlockCatalogEntry } from '../../api/block-catalog';
import {
  discoverTrustedFrontendContribution,
  prepareTrustedFrontendContribution,
  TrustedFrontendContributionLifecycleError
} from '../../lib/native-trusted-block-contribution-lifecycle';

describe('trusted frontend contribution lifecycle', () => {
  test('D5-P2 exposes catalog discovery before preparing the exact graph-backed trusted UiMount binding', () => {
    const discovered = discoverTrustedFrontendContribution(
      catalogEntry(),
      expectation()
    );
    expect(discovered.state).toBe('discovered');

    const contribution = discovered.prepare();
    expect(contribution.state).toBe('prepared');
    const handle = contribution.createHandle();
    const mount = vi.fn();
    const dispose = vi.fn();

    expect(handle.state).toBe('prepared');
    handle.mount({ mount, dispose });
    expect(handle.state).toBe('mounted');
    handle.update();
    expect(handle.state).toBe('updated');
    expect(mount).toHaveBeenCalledOnce();

    handle.dispose();
    expect(handle.state).toBe('disposed');
    expect(dispose).toHaveBeenCalledOnce();
    expect(contribution).toMatchObject({
      contributionId: 'frontend-block.installation-1.hero_banner',
      blockVersion: '1.0.0',
      assetIntegrity: ['verified_sha256'],
      grantedPermissions: ['frontend-block.ui-mount.trusted-host'],
      graphFingerprint: 'graph-fingerprint',
      runtimeKind: 'trusted_native',
      executionKind: 'ui_mount',
      isolationRequirement: 'trusted_host_realm'
    });
  });

  test('D5-P2 accepts repeated updates while mounted or updated without remounting', () => {
    const handle = prepareTrustedFrontendContribution(
      catalogEntry(),
      expectation()
    ).createHandle();
    const mount = vi.fn();
    const dispose = vi.fn();
    handle.mount({ mount, dispose });

    handle.update();
    handle.update();
    handle.update();

    expect(handle.state).toBe('updated');
    expect(mount).toHaveBeenCalledOnce();
    expect(dispose).not.toHaveBeenCalled();
  });

  test('D5-P2 makes dispose idempotent while keeping other invalid transitions deterministic', () => {
    const handle = prepareTrustedFrontendContribution(
      catalogEntry(),
      expectation()
    ).createHandle();
    const dispose = vi.fn();
    handle.mount({ mount: vi.fn(), dispose });

    expect(() => handle.mount({ mount: vi.fn(), dispose: vi.fn() })).toThrow(
      TrustedFrontendContributionLifecycleError
    );
    handle.dispose();
    expect(handle.state).toBe('disposed');
    expect(() => handle.update()).toThrowError(/lifecycle is disposed/u);
    expect(() => handle.dispose()).not.toThrow();
    expect(() => handle.dispose()).not.toThrow();
    expect(dispose).toHaveBeenCalledOnce();
  });

  test('D5-P2 retains failed state, attempts cleanup, and preserves the original mount error', () => {
    const handle = prepareTrustedFrontendContribution(
      catalogEntry(),
      expectation()
    ).createHandle();
    const mountError = new Error('controlled mount failure');
    const dispose = vi.fn(() => {
      throw new Error('controlled disposer failure');
    });
    let caught: unknown;

    try {
      handle.mount({
        mount: () => {
          throw mountError;
        },
        dispose
      });
    } catch (error) {
      caught = error;
    }

    expect(caught).toBe(mountError);
    expect(handle.state).toBe('failed');
    expect(dispose).toHaveBeenCalledOnce();
    expect(() => handle.dispose()).not.toThrow();
    expect(handle.state).toBe('failed');
    expect(dispose).toHaveBeenCalledOnce();
  });

  test.each([
    ['runtime', { runtime_kind: 'isolated' }],
    ['execution', { execution_kind: 'worker' }],
    ['realm', { isolation_requirement: 'independent_realm' }],
    ['permission', { granted_permissions: [] }],
    [
      'graph provenance',
      {
        provenance: {
          module_id: 'x',
          module_version: '1',
          module_kind: 'capability'
        }
      }
    ],
    ['disabled receipt', { disable_reason: 'permission_denied' }]
  ])('D5-P2 rejects incompatible %s metadata', (_label, overrides) => {
    expect(() =>
      prepareTrustedFrontendContribution(
        catalogEntry(overrides as Partial<FrontstageBlockCatalogEntry>),
        expectation()
      )
    ).toThrow(TrustedFrontendContributionLifecycleError);
  });

  test('D5-P2 rejects an asset without the verified D5-P1 integrity receipt', () => {
    const invalid = catalogEntry();
    invalid.code_modules[0]!.assets[0] = {
      ...invalid.code_modules[0]!.assets[0]!,
      integrity: 'invalid' as 'verified_sha256'
    };

    expect(() =>
      prepareTrustedFrontendContribution(invalid, expectation())
    ).toThrowError(/asset integrity mismatch/u);
  });

  test('D5-P2 source states instance disposal without claiming Shadow DOM security isolation', () => {
    const lifecycleSource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-trusted-block-contribution-lifecycle.ts'
      ),
      'utf8'
    );
    const adapterSource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-trusted-block-react-adapter.tsx'
      ),
      'utf8'
    );

    expect(lifecycleSource).toContain(
      'does not unload an evaluated JavaScript module or provide a security realm'
    );
    expect(`${lifecycleSource}\n${adapterSource}`).not.toMatch(
      /Shadow(?:Root| DOM).{0,40}(?:security|sandbox|permission isolation)/iu
    );
  });
});

function expectation() {
  return {
    workspaceId: 'workspace-1',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero_banner'
  };
}

function catalogEntry(
  overrides: Partial<FrontstageBlockCatalogEntry> = {}
): FrontstageBlockCatalogEntry {
  const sha256 = 'a'.repeat(64);
  return {
    installation_id: 'installation-1',
    provider_code: 'official',
    plugin_id: 'official.blocks',
    plugin_version: '1.0.0',
    contribution_code: 'hero_banner',
    title: 'Hero Banner',
    runtime: 'native_react',
    entry: 'blocks/hero/index.js',
    code_modules: [
      {
        source: '@1flowbase/native-components',
        version: '1.0.0',
        binding: 'fetched',
        assets: [
          {
            role: 'browser_module',
            media_type: 'text/javascript; charset=utf-8',
            sha256,
            url: `/api/console/frontstage/component-module-assets/${sha256}`,
            integrity: 'verified_sha256'
          }
        ],
        exports: ['Surface'],
        type_declarations: 'export declare const Surface: unknown;'
      }
    ],
    context_contract: { primitives: ['text'], input_schema: {} },
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    ui_capabilities: ['responsive'],
    frontend_contribution_id: 'frontend-block.installation-1.hero_banner',
    frontend_block_id: 'installation-1:hero_banner',
    frontend_block_version: '1.0.0',
    runtime_kind: 'trusted_native',
    execution_kind: 'ui_mount',
    isolation_requirement: 'trusted_host_realm',
    requested_permissions: ['frontend-block.ui-mount.trusted-host'],
    granted_permissions: ['frontend-block.ui-mount.trusted-host'],
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
