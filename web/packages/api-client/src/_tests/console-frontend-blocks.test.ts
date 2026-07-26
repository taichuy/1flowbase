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
          browser_asset: {
            sha256:
              'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
          },
          exports: ['blockSdkVersion'],
          type_declarations: 'export declare function defineBlock(): unknown;'
        }
      ],
      context_contract: { primitives: ['text'], input_schema: {} },
      permissions: { network: 'none', storage: 'none', secrets: 'none' },
      ui_capabilities: ['responsive']
    } satisfies ConsoleFrontendBlockCatalogEntry;

    expect(entry.code_modules?.[0]).toMatchObject({
      source: '@1flowbase/block-sdk',
      version: '1.0.0',
      browser_asset: {
        sha256:
          'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
      },
      exports: ['blockSdkVersion']
    });
  });
});
