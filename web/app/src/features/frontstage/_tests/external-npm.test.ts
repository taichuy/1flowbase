import { describe, expect, test, vi } from 'vitest';

import {
  describeExternalNpmImportFailure,
  fetchExternalNpmPack,
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

  test('AC-001 treats missing, unavailable, and invalid optional packs as isolated states', async () => {
    await expect(
      fetchExternalNpmPack(vi.fn(async () => new Response('', { status: 404 })))
    ).resolves.toEqual({ modules: [], state: { status: 'absent' } });
    await expect(
      fetchExternalNpmPack(
        vi.fn(async () => {
          throw new Error('connect ECONNREFUSED 127.0.0.1:4174');
        })
      )
    ).resolves.toEqual({ modules: [], state: { status: 'unavailable' } });
    await expect(
      fetchExternalNpmPack(
        vi.fn(async () => Response.json({ schema_version: 2, modules: [] }))
      )
    ).resolves.toEqual({ modules: [], state: { status: 'invalid' } });
  });

  test('AC-003 exposes valid optional modules as an available snapshot', async () => {
    await expect(
      fetchExternalNpmPack(vi.fn(async () => Response.json(manifest)))
    ).resolves.toEqual({
      modules: manifest.modules,
      state: { status: 'available' }
    });
  });

  test('keeps backend catalog modules authoritative when the optional pack repeats a source', () => {
    const backendModule = {
      ...manifest.modules[0],
      version: 'backend-version'
    };
    const entry = {
      code_modules: [backendModule],
      runtime: 'native_react'
    } as unknown as FrontstageBlockCatalogEntry;

    expect(
      mergeExternalNpmModules([entry], manifest.modules)[0]?.code_modules
    ).toEqual([backendModule]);
  });

  test('AC-002 and AC-005 preserve import denial while explaining optional-pack state', () => {
    const denied = "Import source 'dayjs' is not allowed.";

    expect(
      describeExternalNpmImportFailure(denied, { status: 'unavailable' })
    ).toBe(
      "Import source 'dayjs' is not allowed. Optional External npm Pack is unavailable."
    );
    expect(
      describeExternalNpmImportFailure(denied, { status: 'available' })
    ).toBe(denied);
    expect(
      describeExternalNpmImportFailure('Native React compilation failed.', {
        status: 'unavailable'
      })
    ).toBe('Native React compilation failed.');
  });
});
