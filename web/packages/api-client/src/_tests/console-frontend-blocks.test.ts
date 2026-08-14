import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  listConsoleFrontendBlocks,
  type ConsoleFrontendBlockCatalogEntry
} from '../console/frontend-blocks';

describe('console-frontend-blocks client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('lists frontend block catalog entries from console endpoint', async () => {
    await expect(listConsoleFrontendBlocks()).resolves.toMatchObject({
      path: '/api/console/frontend-blocks',
      method: 'GET'
    });
  });

  test('exposes the optional code_template contract for runtime agents', () => {
    const entry = {
      installation_id: 'installation-id',
      provider_code: 'provider-code',
      plugin_id: 'plugin-id',
      plugin_version: '0.1.0',
      contribution_code: 'hero_banner',
      title: 'Hero Banner',
      runtime: 'native_react',
      entry: 'blocks/hero/index.html',
      code_template:
        'export default function HeroBanner() { return <section>Hero</section>; }',
      code_template_version: '1.0.0',
      code_template_language: 'tsx',
      code_modules: [
        {
          source: '@1flowbase/block-sdk',
          version: '1.0.0',
          binding: 'fetched',
          assets: [
            {
              role: 'browser_module',
              media_type: 'text/javascript; charset=utf-8',
              sha256:
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
              url: '/api/console/frontstage/workspace-id/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
              integrity: 'verified_sha256'
            }
          ],
          exports: ['blockSdkVersion'],
          type_declarations: 'export declare function defineBlock(): unknown;'
        }
      ],
      context_contract: { primitives: ['text'], input_schema: {} },
      permissions: { network: 'none', storage: 'none', secrets: 'none' },
      ui_capabilities: ['responsive'],
      frontend_contribution_id: 'frontend-block.installation-id.hero_banner',
      frontend_block_id: 'installation-id:hero_banner',
      frontend_block_version: '0.1.0',
      runtime_kind: 'trusted_native',
      execution_kind: 'ui_mount',
      isolation_requirement: 'trusted_host_realm',
      requested_permissions: ['frontend-block.ui-mount.trusted-host'],
      granted_permissions: ['frontend-block.ui-mount.trusted-host'],
      workspace_id: 'workspace-id',
      lifecycle_kind: 'workspace_assignment',
      graph_fingerprint: 'graph-fingerprint',
      provenance: {
        module_id: 'boot-core',
        module_version: '1',
        module_kind: 'boot_core'
      },
      disable_reason: null
    } satisfies ConsoleFrontendBlockCatalogEntry;

    expect(entry.code_modules?.[0]).toMatchObject({
      source: '@1flowbase/block-sdk',
      version: '1.0.0',
      binding: 'fetched',
      assets: [
        {
          role: 'browser_module',
          media_type: 'text/javascript; charset=utf-8',
          sha256:
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          url: '/api/console/frontstage/workspace-id/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          integrity: 'verified_sha256'
        }
      ],
      exports: ['blockSdkVersion']
    });
  });
});
