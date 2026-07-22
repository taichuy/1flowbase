import { describe, expect, test } from 'vitest';

import {
  FRONTEND_BLOCK_CONTEXT_PRIMITIVES,
  FRONTEND_BLOCK_PACKAGE_BOUNDARIES,
  FRONTEND_BLOCK_RUNTIMES,
  FRONTEND_BLOCK_UI_CAPABILITIES,
  createFrontendBlockCodeCapabilities,
  isFrontendBlockContextPrimitive,
  isFrontendBlockPackageBoundary,
  isFrontendBlockRuntime,
  isFrontendBlockUiCapability,
  normalizeFrontendBlockContextPrimitive,
  normalizeFrontendBlockContextPrimitives,
  normalizeFrontendBlockRuntime,
  normalizeFrontendBlockUiCapabilities,
  normalizeFrontendBlockUiCapability
} from '../index';

describe('frontend block contract vocabulary', () => {
  test('projects catalog modules into editor, import, and worker capabilities', () => {
    expect(
      createFrontendBlockCodeCapabilities({
        code_template: 'export default {}',
        code_template_version: '1.0.0',
        code_template_language: 'tsx',
        code_modules: [
          {
            source: '@1flowbase/block-sdk',
            type_declarations: 'export declare function defineBlock(): unknown;'
          }
        ]
      })
    ).toEqual({
      template: {
        source: 'export default {}',
        version: '1.0.0',
        language: 'tsx'
      },
      allowedImports: ['@1flowbase/block-sdk'],
      monacoExtraLibs: [
        {
          source: '@1flowbase/block-sdk',
          filePath: 'file:///node_modules/@1flowbase/block-sdk/index.d.ts',
          content: 'export declare function defineBlock(): unknown;'
        }
      ],
      workerModuleSources: ['@1flowbase/block-sdk']
    });
  });
  test('exports the backend-aligned manifest and catalog vocabularies', () => {
    expect(FRONTEND_BLOCK_RUNTIMES).toEqual(['iframe']);
    expect(FRONTEND_BLOCK_CONTEXT_PRIMITIVES).toEqual([
      'text',
      'image',
      'link',
      'button',
      'rich_text',
      'data_record'
    ]);
    expect(FRONTEND_BLOCK_UI_CAPABILITIES).toEqual([
      'responsive',
      'configurable',
      'theming',
      'data_binding'
    ]);
  });

  test('exports only the current frontend block package boundary', () => {
    expect(FRONTEND_BLOCK_PACKAGE_BOUNDARIES).toEqual([
      'page-protocol',
      'page-runtime',
      'block-renderer',
      'block-sdk',
      'antd-facade'
    ]);
    expect(FRONTEND_BLOCK_PACKAGE_BOUNDARIES).not.toContain(
      'frontend-block-runtime'
    );
    expect(FRONTEND_BLOCK_PACKAGE_BOUNDARIES).not.toContain('ui-schema');
  });

  test('guards runtime, context primitives, ui capabilities, and package boundaries', () => {
    expect(isFrontendBlockRuntime('iframe')).toBe(true);
    expect(isFrontendBlockRuntime('worker')).toBe(false);
    expect(isFrontendBlockRuntime(null)).toBe(false);

    expect(isFrontendBlockContextPrimitive('rich_text')).toBe(true);
    expect(isFrontendBlockContextPrimitive('chart')).toBe(false);

    expect(isFrontendBlockUiCapability('data_binding')).toBe(true);
    expect(isFrontendBlockUiCapability('scripting')).toBe(false);

    expect(isFrontendBlockPackageBoundary('page-runtime')).toBe(true);
    expect(isFrontendBlockPackageBoundary('frontend-block-runtime')).toBe(
      false
    );
  });

  test('normalizes single contract values without widening unknown strings', () => {
    expect(normalizeFrontendBlockRuntime('iframe')).toBe('iframe');
    expect(normalizeFrontendBlockRuntime('worker')).toBeNull();
    expect(normalizeFrontendBlockContextPrimitive('data_record')).toBe(
      'data_record'
    );
    expect(normalizeFrontendBlockContextPrimitive('markdown')).toBeNull();
    expect(normalizeFrontendBlockUiCapability('theming')).toBe('theming');
    expect(normalizeFrontendBlockUiCapability('analytics')).toBeNull();
  });

  test('normalizes list values by filtering unknown entries and de-duplicating', () => {
    expect(
      normalizeFrontendBlockContextPrimitives([
        'text',
        'chart',
        'text',
        'image',
        null,
        'data_record'
      ])
    ).toEqual(['text', 'image', 'data_record']);

    expect(
      normalizeFrontendBlockUiCapabilities([
        'responsive',
        'scripting',
        'responsive',
        'data_binding',
        'theming'
      ])
    ).toEqual(['responsive', 'data_binding', 'theming']);

    expect(normalizeFrontendBlockContextPrimitives('text')).toEqual([]);
    expect(normalizeFrontendBlockUiCapabilities(undefined)).toEqual([]);
  });
});
