import { describe, expect, test } from 'vitest';

import {
  createDefaultJsBlockWorkerExecutor,
  validateJsBlockSource
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

const allowedImportSources = [
  '@1flowbase/block-sdk',
  '@1flowbase/block-renderer/antd-facade'
];

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
    expect(nextTemplates[0].title).toBe('JSX 示例区块');
  });

  test('creates built-in JS block source by stable template id', () => {
    for (const template of listFrontstageBuiltInJsBlockTemplates()) {
      const code = createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: template.id
      });

      expect(code).toContain('@1flowbase/block-sdk');
      expect(code).toContain('@1flowbase/block-renderer/antd-facade');
      expect(code).toContain('defineBlock');
      expect(code).toContain(`blockId: '${templateInput.blockId}'`);
      expect(code).toContain(`codeRef: '${templateInput.codeRef}'`);
      expect(code).toContain(
        `contributionCode: '${templateInput.contributionCode}'`
      );
      expect(code).toContain(`id: '${templateInput.blockId}'`);
      expect(code).toContain(`title: '${template.title}'`);
      expect(validateJsBlockSource(code)).toMatchObject({ ok: true });
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

  test('keeps data, state, event, and action examples inside the matching templates', () => {
    const snippetsByTemplateId = {
      blank: [],
      'data-table': ['ctx.data.query', 'ctx.patch', 'ctx.events.emit'],
      'create-form': ['ctx.actions.invoke', 'ctx.patch', 'ctx.events.emit'],
      'edit-form': ['ctx.data.query', 'ctx.patch', 'ctx.actions.invoke'],
      'search-table': ['ctx.data.query', 'ctx.patch', 'ctx.events.emit']
    } satisfies Record<
      (typeof FRONTSTAGE_BUILT_IN_JS_BLOCK_TEMPLATES)[number]['id'],
      string[]
    >;

    for (const template of listFrontstageBuiltInJsBlockTemplates()) {
      const code = createFrontstageBuiltInJsBlockTemplateCode({
        ...templateInput,
        templateId: template.id
      });

      for (const snippet of snippetsByTemplateId[template.id]) {
        expect(code).toContain(snippet);
      }
    }
  });

  test('creates a minimal JSX example block with the selected block refs', () => {
    const code = createBlankJsBlockTemplateCode({
      blockId: 'frontstage-js-block-1',
      codeRef: 'frontstage-js-block-1-code',
      contributionCode: 'frontstage.js-ui-block'
    });

    expect(code).toContain('@1flowbase/block-sdk');
    expect(code).toContain('@1flowbase/block-renderer/antd-facade');
    expect(code).toContain('defineBlock');
    expect(code).toContain("blockId: 'frontstage-js-block-1'");
    expect(code).toContain("codeRef: 'frontstage-js-block-1-code'");
    expect(code).toContain("contributionCode: 'frontstage.js-ui-block'");
    expect(code).toContain("id: 'frontstage-js-block-1'");
    expect(code).toContain("title: 'JSX 示例区块'");
    expect(code).toContain('render()');
    expect(code).toContain('<Title>JSX 示例区块</Title>');
    expect(code).not.toContain('ctx.data.query');
    expect(code).not.toContain('ctx.actions.invoke');
  });

  test('runs the generated blank JS block through the default worker executor', async () => {
    const source = createBlankJsBlockTemplateCode({
      blockId: 'frontstage-js-block-1',
      codeRef: 'frontstage-js-block-1-code',
      contributionCode: 'frontstage.js-ui-block'
    });

    const executor = createDefaultJsBlockWorkerExecutor();

    const messages = await executor.handleMessage({
      direction: 'host_to_worker',
      type: 'run',
      request: {
        requestId: 'request-1',
        blockId: 'frontstage-js-block-1',
        source,
        props: {},
        state: {},
        contextSnapshot: {
          workspace: { id: 'workspace-1' },
          application: { id: 'application-1' },
          page: { id: 'page-1', route: '/frontstage/page-1' }
        },
        limits: {
          timeoutMs: 1000,
          maxRenderDepth: 8,
          maxRenderNodes: 250
        }
      }
    });

    expect(messages).toEqual([
      {
        direction: 'worker_to_host',
        type: 'rendered',
        requestId: 'request-1',
        schema: {
          primitive: 'Stack',
          children: [
            {
              primitive: 'Title',
              props: { children: 'JSX 示例区块' }
            },
            {
              primitive: 'Text',
              props: {
                children: '点击区块右上角的编辑图标修改这段 JSX。'
              }
            }
          ]
        }
      }
    ]);

    expect(source).not.toContain('Card');
    expect(source).not.toContain('Space');
    expect(source).not.toContain('Typography');
    expect(source).not.toContain('meta:');
    expect(source).not.toContain('ctx.state(');
  });
});
