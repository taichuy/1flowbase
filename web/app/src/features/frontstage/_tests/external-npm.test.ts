import { describe, expect, test, vi } from 'vitest';

import {
  fetchExternalNpmModules,
  mergeExternalNpmModules,
  normalizeExternalNpmManifest
} from '../api/external-npm';
import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';

const sha256 = 'a'.repeat(64);
const manifest = {
  schema_version: 1,
  modules: [
    {
      source: 'tailwindcss',
      version: '4.3.3',
      binding: 'fetched' as const,
      assets: [
        {
          role: 'browser_module' as const,
          media_type: 'text/javascript; charset=utf-8',
          sha256,
          url: `/external-npm/assets/tailwindcss-${sha256}.js`
        },
        {
          role: 'shadow_style' as const,
          media_type: 'text/css; charset=utf-8',
          sha256,
          url: `/external-npm/assets/tailwindcss-${sha256}.css`
        }
      ],
      exports: ['default'],
      type_declarations:
        'declare module "tailwindcss" { const value: unknown; export default value; }'
    }
  ]
};

describe('external npm manifest', () => {
  test('AC-003 normalizes the current static manifest without inventing versions or URLs', () => {
    expect(normalizeExternalNpmManifest(manifest)).toEqual(manifest.modules);
    expect(() =>
      normalizeExternalNpmManifest({
        ...manifest,
        modules: [
          {
            ...manifest.modules[0],
            assets: [{ ...manifest.modules[0].assets[0], url: '/wrong.js' }]
          }
        ]
      })
    ).toThrow(/external npm manifest/iu);
  });

  test('AC-005 merges external imports into each backend catalog entry', () => {
    const entry = {
      code_modules: [],
      runtime: 'native_react'
    } as unknown as FrontstageBlockCatalogEntry;

    expect(
      mergeExternalNpmModules([entry], manifest.modules)[0]?.code_modules
    ).toEqual(manifest.modules);
  });

  test('treats a missing optional pack as empty but rejects malformed published content', async () => {
    await expect(
      fetchExternalNpmModules(
        vi.fn(async () => new Response('', { status: 404 }))
      )
    ).resolves.toEqual([]);
    await expect(
      fetchExternalNpmModules(
        vi.fn(async () => Response.json({ schema_version: 2, modules: [] }))
      )
    ).rejects.toThrow(/external npm manifest/iu);
  });
});
