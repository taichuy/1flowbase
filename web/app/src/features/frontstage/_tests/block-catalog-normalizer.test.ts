import { describe, expect, test } from 'vitest';

import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';
import {
  filterFrontstageBlockCatalogByCapability,
  filterFrontstageBlockCatalogByRuntime,
  hasFrontstageBlockActionPermission,
  hasFrontstageBlockDataPermission,
  hasFrontstageBlockEventPermission,
  isFrontstageBlockIframeRuntime,
  isFrontstageBlockRestrictedRuntime,
  normalizeFrontstageBlockCatalog,
  supportsFrontstageBlockCapability,
  supportsFrontstageBlockPrimitive
} from '../lib/block-catalog';

function createCatalogEntry(
  overrides: Partial<FrontstageBlockCatalogEntry> = {}
): FrontstageBlockCatalogEntry {
  return {
    installation_id: 'installation-1',
    provider_code: 'official',
    plugin_id: 'official.blocks',
    plugin_version: '1.0.0',
    contribution_code: 'hero_banner',
    title: 'Hero Banner',
    runtime: 'iframe',
    entry: 'blocks/hero/index.html',
    context_contract: {
      primitives: ['text'],
      input_schema: { type: 'object' }
    },
    permissions: {
      network: 'outbound_only',
      storage: 'none',
      secrets: 'none'
    },
    ui_capabilities: ['responsive'],
    ...overrides
  };
}

describe('frontstage block catalog normalizer', () => {
  test('normalizes console catalog entries into frontstage runtime models', () => {
    const result = normalizeFrontstageBlockCatalog([
      createCatalogEntry({
        context_contract: {
          primitives: ['text', 'data_record', 'button'],
          input_schema: {
            type: 'object',
            properties: { title: { type: 'string' } }
          }
        },
        ui_capabilities: ['responsive', 'data_binding']
      })
    ]);

    expect(result.diagnostics).toEqual([]);
    expect(result.items).toEqual([
      {
        id: 'official:hero_banner',
        runtimeKind: 'iframe',
        installationId: 'installation-1',
        providerCode: 'official',
        pluginId: 'official.blocks',
        pluginVersion: '1.0.0',
        contributionCode: 'hero_banner',
        title: 'Hero Banner',
        entry: 'blocks/hero/index.html',
        permissions: {
          network: 'outbound_only',
          storage: 'none',
          secrets: 'none'
        },
        contextContract: {
          primitives: ['text', 'data_record', 'button'],
          inputSchema: {
            type: 'object',
            properties: { title: { type: 'string' } }
          }
        },
        uiCapabilities: ['responsive', 'data_binding'],
        codeModules: [],
        raw: expect.any(Object)
      }
    ]);

    const [block] = result.items;
    expect(isFrontstageBlockIframeRuntime(block)).toBe(true);
    expect(isFrontstageBlockRestrictedRuntime(block)).toBe(true);
    expect(supportsFrontstageBlockCapability(block, 'data_binding')).toBe(true);
    expect(supportsFrontstageBlockPrimitive(block, 'button')).toBe(true);
    expect(hasFrontstageBlockDataPermission(block)).toBe(true);
    expect(hasFrontstageBlockActionPermission(block)).toBe(true);
    expect(hasFrontstageBlockEventPermission(block)).toBe(false);
  });

  test('D2-AC-001 retains canonical module version, digest, and declarations', () => {
    const { items } = normalizeFrontstageBlockCatalog([
      createCatalogEntry({
        code_modules: [
          {
            source: '@1flowbase/native-components',
            version: '1.0.0',
            browser_asset: {
              sha256:
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
            },
            type_declarations:
              "declare module '@1flowbase/native-components' { export const Surface: import('react').ComponentType<SurfaceProps>; }"
          }
        ]
      })
    ]);

    expect(items[0]?.codeModules).toEqual([
      {
        source: '@1flowbase/native-components',
        version: '1.0.0',
        browser_asset: {
          sha256:
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        },
        type_declarations: expect.stringContaining(
          "import('react').ComponentType<SurfaceProps>"
        )
      }
    ]);
  });

  test('filters unknown runtime entries and reports diagnostics', () => {
    const result = normalizeFrontstageBlockCatalog([
      createCatalogEntry({ runtime: 'react_remote' })
    ]);

    expect(result.items).toEqual([]);
    expect(result.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'unknown_runtime',
        providerCode: 'official',
        pluginId: 'official.blocks',
        contributionCode: 'hero_banner',
        field: 'runtime',
        value: 'react_remote',
        message:
          'Unsupported frontstage block runtime "react_remote"; entry was filtered.'
      }
    ]);
  });

  test('keeps entries but filters unknown primitives and capabilities', () => {
    const result = normalizeFrontstageBlockCatalog([
      createCatalogEntry({
        context_contract: {
          primitives: ['text', 'script'],
          input_schema: { type: 'object' }
        },
        ui_capabilities: ['responsive', 'arbitrary_dom_access']
      })
    ]);

    expect(result.items).toHaveLength(1);
    expect(result.items[0].contextContract.primitives).toEqual(['text']);
    expect(result.items[0].uiCapabilities).toEqual(['responsive']);
    expect(result.diagnostics).toEqual([
      expect.objectContaining({
        severity: 'warning',
        code: 'unknown_primitive',
        field: 'context_contract.primitives',
        value: 'script'
      }),
      expect.objectContaining({
        severity: 'warning',
        code: 'unknown_capability',
        field: 'ui_capabilities',
        value: 'arbitrary_dom_access'
      })
    ]);
  });

  test('filters normalized entries by runtime and capability', () => {
    const { items } = normalizeFrontstageBlockCatalog([
      createCatalogEntry({
        contribution_code: 'hero_banner',
        ui_capabilities: ['responsive']
      }),
      createCatalogEntry({
        contribution_code: 'product_grid',
        ui_capabilities: ['responsive', 'configurable']
      })
    ]);

    expect(
      filterFrontstageBlockCatalogByRuntime(items, 'iframe').map(
        (item) => item.contributionCode
      )
    ).toEqual(['hero_banner', 'product_grid']);
    expect(
      filterFrontstageBlockCatalogByCapability(items, 'configurable').map(
        (item) => item.contributionCode
      )
    ).toEqual(['product_grid']);
  });
});
