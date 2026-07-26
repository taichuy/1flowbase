import { describe, expect, test } from 'vitest';

import {
  compileNativeReactComponent,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';

import {
  FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES,
  createBlankJsBlockTemplateCode,
  createFrontstageBuiltInJsBlockTemplateCode,
  listFrontstageBuiltInJsBlockTemplates,
  type FrontstageBuiltInJsBlockTemplate,
  type FrontstageBuiltInJsBlockTemplateId
} from '../../lib/block-templates';

const templateInput = {
  blockId: 'frontstage-js-block-1',
  codeRef: 'frontstage-js-block-1-code',
  contributionCode: 'frontstage.js-ui-block'
};

const allowedImportSources = ['react'];

const forbiddenSourceFragments = [
  '@1flowbase/antd-facade',
  'window',
  'document',
  'fetch',
  'localStorage',
  'sessionStorage'
];

function readImportSources(source: string): string[] {
  return Array.from(source.matchAll(/from\s+['"]([^'"]+)['"]/g)).map(
    (match) => match[1]
  );
}

describe('frontstage block templates', () => {
  test('lists the stable built-in JS block template registry', () => {
    expect(
      FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES.map((template) => template.id)
    ).toEqual([
      'blank',
      'data-table',
      'create-form',
      'edit-form',
      'search-table'
    ]);

    expect(listFrontstageBuiltInJsBlockTemplates()).toEqual(
      FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES
    );
  });

  test('does not expose the mutable built-in template registry internals', () => {
    const listedTemplates =
      listFrontstageBuiltInJsBlockTemplates() as FrontstageBuiltInJsBlockTemplate[];

    listedTemplates.reverse();
    listedTemplates[0].title = 'Mutated Template';

    const nextTemplates = listFrontstageBuiltInJsBlockTemplates();
    expect(nextTemplates.map((template) => template.id)).toEqual([
      'blank',
      'data-table',
      'create-form',
      'edit-form',
      'search-table'
    ]);
    expect(nextTemplates[0].title).toBe('代码示例区块');
  });

  test('creates built-in JS block source by stable template id', () => {
    for (const template of listFrontstageBuiltInJsBlockTemplates()) {
      const code = createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: template.id
      });

      expect(code).toContain("from 'react'");
      expect(code).toContain('export default function Block(');
      expect(code).toContain('<style>');
      expect(code).toContain('<button');
      expect(code).not.toContain('defineBlock');
      expect(code).not.toContain(templateInput.blockId);
      expect(validateNativeTrustedBlockSource(code)).toMatchObject({
        ok: true
      });
    }
  });

  test('rejects unknown built-in JS block template ids with a clear error', () => {
    expect(() =>
      createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: 'unknown-template' as FrontstageBuiltInJsBlockTemplateId
      })
    ).toThrow(
      'Unknown FrontStage built-in JS block template: unknown-template'
    );
  });

  test('keeps the legacy blank generator compatible with the unified generator', () => {
    expect(createBlankJsBlockTemplateCode(templateInput)).toBe(
      createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: 'blank'
      })
    );
  });

  test('uses only allowed JS block imports and avoids denied runtime escapes', () => {
    for (const template of listFrontstageBuiltInJsBlockTemplates()) {
      const code = createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: template.id
      });

      expect(readImportSources(code)).toEqual(allowedImportSources);
      for (const fragment of forbiddenSourceFragments) {
        expect(code).not.toContain(fragment);
      }
    }
  });

  test('keeps interface code out of templates until the user binds and inserts it', () => {
    for (const template of listFrontstageBuiltInJsBlockTemplates()) {
      const code = createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: template.id
      });

      expect(code).not.toContain('ctx.data');
      expect(code).not.toContain('ctx.actions');
      expect(code).not.toContain('ctx.interfaces.call');
    }
  });

  test('creates a minimal JSX example block with the selected block refs', () => {
    const code = createBlankJsBlockTemplateCode({
      blockId: 'frontstage-js-block-1',
      codeRef: 'frontstage-js-block-1-code',
      contributionCode: 'frontstage.js-ui-block'
    });

    expect(code).toContain("import { useState } from 'react'");
    expect(code).toContain('export default function Block(');
    expect(code).toContain('@1flowbase-context');
    expect(code).toContain('代码示例区块</div>');
    expect(code).not.toContain('ctx.data.query');
    expect(code).not.toContain('ctx.actions.invoke');
  });

  test('compiles the generated blank block as a standard Native React artifact', () => {
    const source = createBlankJsBlockTemplateCode({
      blockId: 'frontstage-js-block-1',
      codeRef: 'frontstage-js-block-1-code',
      contributionCode: 'frontstage.js-ui-block'
    });

    const compiled = compileNativeReactComponent(source);
    expect(compiled.ok).toBe(true);
    if (compiled.ok) {
      expect(compiled.artifact.program.executableBody).toContain("'button'");
      expect(compiled.artifact.sourceMap).toMatchObject({ version: 3 });
    }

    expect(source).not.toContain('Card');
    expect(source).not.toContain('Space');
    expect(source).not.toContain('Typography');
    expect(source).not.toContain('meta:');
    expect(source).not.toContain('ctx.state(');
  });
});
