import { describe, expect, test } from 'vitest';

import type { FrontstagePageContent } from '../api/page-content';
import {
  createFrontstagePageDocument,
  createFrontstagePageDocumentSaveInput,
  type FrontstageBlockInstance
} from '../lib/page-document';

function createPageContent(
  overrides: Partial<FrontstagePageContent> = {}
): FrontstagePageContent {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
    },
    schema: {
      rootUid: 'root-1',
      payload: {}
    },
    root: {
      uid: 'root-1',
      payload: {}
    },
    ...overrides
  };
}

describe('frontstage page document', () => {
  test('normalizes an empty content payload into an empty document', () => {
    const document = createFrontstagePageDocument(createPageContent());

    expect(document.page.id).toBe('page-1');
    expect(document.rootUid).toBe('root-1');
    expect(document.blocks).toEqual([]);
    expect(document.isEmpty).toBe(true);
    expect(document.diagnostics).toEqual([]);
  });

  test('normalizes valid block instances from the root payload', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'hero',
                codeRef: 'hero-code',
                catalog: {
                  providerCode: 'official',
                  installationId: 'installation-1'
                },
                contribution: {
                  pluginId: 'official.blocks',
                  pluginVersion: '1.0.0',
                  code: 'official.hero'
                },
                props: { title: 'Hello' },
                'x-layout': { region: 'main', order: 20, span: 12 },
                runtime: { kind: 'iframe', entry: 'blocks/hero.html' }
              }
            ]
          }
        }
      })
    );

    expect(document.isEmpty).toBe(false);
    expect(document.blocks).toEqual([
      {
        id: 'hero',
        sourceId: 'hero',
        codeRef: 'hero-code',
        sourceCodeRef: 'hero-code',
        catalog: {
          providerCode: 'official',
          installationId: 'installation-1'
        },
        contribution: {
          pluginId: 'official.blocks',
          pluginVersion: '1.0.0',
          code: 'official.hero'
        },
        props: { title: 'Hello' },
        layout: { region: 'main', order: 20, span: 12 },
        order: 20,
        runtime: {
          kind: 'iframe',
          entry: 'blocks/hero.html',
          hint: 'iframe'
        }
      }
    ]);
    expect(document.diagnostics).toEqual([]);
  });

  test('prefers x-layout over legacy layout when both are present', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'hero',
                codeRef: 'hero-code',
                contributionCode: 'official.hero',
                layout: { region: 'legacy', order: 99, width: 1, height: 1 },
                'x-layout': {
                  region: 'main',
                  order: 20,
                  width: 12,
                  height: 4
                },
                runtime: 'inline'
              }
            ]
          }
        }
      })
    );

    expect(document.blocks[0].layout).toEqual({
      region: 'main',
      order: 20,
      width: 12,
      height: 4
    });
    expect(document.blocks[0].order).toBe(20);
    expect(document.diagnostics).toEqual([]);
  });

  test('falls back to legacy layout when x-layout is missing', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'hero',
                codeRef: 'hero-code',
                contributionCode: 'official.hero',
                layout: { region: 'legacy', order: 7, width: 8, height: 2 },
                runtime: 'inline'
              }
            ]
          }
        }
      })
    );

    expect(document.blocks[0].layout).toEqual({
      region: 'legacy',
      order: 7,
      width: 8,
      height: 2
    });
    expect(document.blocks[0].order).toBe(7);
    expect(document.diagnostics).toEqual([]);
  });

  test('falls back to schema blocks when root payload has no block array', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        schema: {
          rootUid: 'root-1',
          payload: {
            blocks: [
              {
                id: 'schema-block',
                code_ref: 'schema-code',
                contribution_code: 'official.schema',
                runtime: 'inline'
              }
            ]
          }
        },
        root: {
          uid: 'root-1',
          payload: { kind: 'frontstage.page.root' }
        }
      })
    );

    expect(document.blocks).toHaveLength(1);
    expect(document.blocks[0]).toMatchObject({
      id: 'schema-block',
      codeRef: 'schema-code',
      contribution: { code: 'official.schema' },
      runtime: { kind: 'inline', hint: 'inline' }
    });
    expect(document.diagnostics).toEqual([]);
  });

  test('records diagnostics and returns an empty fallback for invalid payloads', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: { uid: 'root-1', payload: 'not-json-object' },
        schema: { rootUid: 'root-1', payload: 42 }
      })
    );

    expect(document.blocks).toEqual([]);
    expect(document.isEmpty).toBe(true);
    expect(document.diagnostics).toEqual([
      {
        severity: 'error',
        code: 'invalid_payload',
        path: 'root.payload',
        message: 'Frontstage root payload must be an object.'
      },
      {
        severity: 'error',
        code: 'invalid_payload',
        path: 'schema.payload',
        message: 'Frontstage schema payload must be an object.'
      }
    ]);
  });

  test('creates stable fallbacks for missing block fields', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [{ props: 'invalid-props', 'x-layout': 'invalid-layout' }]
          }
        }
      })
    );

    expect(document.blocks).toEqual([
      {
        id: 'block-1',
        sourceId: null,
        codeRef: 'block-1-code',
        sourceCodeRef: null,
        catalog: {
          providerCode: null,
          installationId: null
        },
        contribution: {
          pluginId: null,
          pluginVersion: null,
          code: 'unknown'
        },
        props: {},
        layout: { order: 0 },
        order: 0,
        runtime: {
          kind: 'unknown',
          entry: null,
          hint: 'unknown'
        }
      }
    ]);
    expect(document.diagnostics.map((diagnostic) => diagnostic.code)).toEqual([
      'missing_block_id',
      'missing_code_ref',
      'missing_contribution',
      'invalid_block_props',
      'invalid_block_layout',
      'missing_runtime'
    ]);
  });

  test('keeps block instance ids and code refs stable when duplicates appear', () => {
    const document = createFrontstagePageDocument(
      createPageContent({
        root: {
          uid: 'root-1',
          payload: {
            blocks: [
              { id: 'hero', codeRef: 'hero-code', contributionCode: 'hero' },
              { id: 'hero', codeRef: 'hero-code', contributionCode: 'hero' }
            ]
          }
        }
      })
    );

    expect(document.blocks.map((block) => block.id)).toEqual([
      'hero',
      'hero-2'
    ]);
    expect(document.blocks.map((block) => block.codeRef)).toEqual([
      'hero-code',
      'hero-code-2'
    ]);
    expect(document.diagnostics.map((diagnostic) => diagnostic.code)).toEqual([
      'duplicate_block_id',
      'duplicate_code_ref',
      'missing_runtime',
      'missing_runtime'
    ]);
  });

  test('creates save payloads for empty documents while preserving non-block fields', () => {
    const content = createPageContent({
      schema: {
        rootUid: 'root-1',
        payload: {
          version: 1,
          schemaMeta: { owner: 'frontstage' }
        }
      },
      root: {
        uid: 'root-1',
        payload: {
          kind: 'frontstage.page.root',
          rootMeta: ['keep']
        }
      }
    });
    const document = createFrontstagePageDocument(content);

    const input = createFrontstagePageDocumentSaveInput(content, document);

    expect(input).toEqual({
      schema: {
        payload: {
          version: 1,
          schemaMeta: { owner: 'frontstage' },
          blocks: []
        }
      },
      root: {
        payload: {
          kind: 'frontstage.page.root',
          rootMeta: ['keep'],
          blocks: []
        }
      }
    });
  });

  test('serializes current blocks without runtime-only document fields', () => {
    const content = createPageContent({
      schema: {
        rootUid: 'root-1',
        payload: {
          version: 1,
          blocks: [{ id: 'stale-schema-block', codeRef: 'stale-schema-code' }]
        }
      },
      root: {
        uid: 'root-1',
        payload: {
          kind: 'frontstage.page.root',
          blocks: [{ id: 'stale-root-block', codeRef: 'stale-root-code' }]
        }
      }
    });
    const block: FrontstageBlockInstance = {
      id: 'hero',
      sourceId: 'stale-root-block',
      codeRef: 'hero-code',
      sourceCodeRef: 'stale-root-code',
      catalog: {
        providerCode: 'official',
        installationId: 'installation-1'
      },
      contribution: {
        pluginId: 'official.blocks',
        pluginVersion: '1.0.0',
        code: 'official.hero'
      },
      props: { title: 'Hello' },
      layout: { region: 'main', order: 99, span: 12, width: 12, height: 4 },
      order: 3,
      runtime: {
        kind: 'iframe',
        entry: 'blocks/hero.html',
        hint: 'iframe'
      }
    };
    const document = {
      ...createFrontstagePageDocument(content),
      blocks: [block],
      isEmpty: false,
      diagnostics: [
        {
          severity: 'warning' as const,
          code: 'duplicate_block_id',
          path: 'blocks.0',
          message: 'diagnostic only'
        }
      ]
    };

    const input = createFrontstagePageDocumentSaveInput(content, document);

    const expectedBlock = {
      id: 'hero',
      codeRef: 'hero-code',
      catalog: {
        providerCode: 'official',
        installationId: 'installation-1'
      },
      contribution: {
        pluginId: 'official.blocks',
        pluginVersion: '1.0.0',
        code: 'official.hero'
      },
      props: { title: 'Hello' },
      'x-layout': {
        region: 'main',
        order: 3,
        span: 12,
        width: 12,
        height: 4
      },
      runtime: {
        kind: 'iframe',
        entry: 'blocks/hero.html',
        hint: 'iframe'
      }
    };

    expect(input.schema.payload).toEqual({
      version: 1,
      blocks: [expectedBlock]
    });
    expect(input.root.payload).toEqual({
      kind: 'frontstage.page.root',
      blocks: [expectedBlock]
    });
    expect(input.root.payload).not.toHaveProperty('diagnostics');
    expect(input.root.payload).not.toHaveProperty('isEmpty');
    expect(input.root.payload).not.toHaveProperty('sourceId');
    expect(input.root.payload).not.toHaveProperty('sourceCodeRef');
    expect(expectedBlock).not.toHaveProperty('sourceId');
    expect(expectedBlock).not.toHaveProperty('sourceCodeRef');
    expect(expectedBlock).not.toHaveProperty('layout');

    const roundTripDocument = createFrontstagePageDocument(
      createPageContent({
        schema: {
          rootUid: 'root-1',
          payload: input.schema.payload
        },
        root: {
          uid: 'root-1',
          payload: input.root.payload
        }
      })
    );

    expect(roundTripDocument.blocks).toEqual([
      {
        ...block,
        sourceId: 'hero',
        sourceCodeRef: 'hero-code',
        layout: {
          region: 'main',
          order: 3,
          span: 12,
          width: 12,
          height: 4
        },
        order: 3
      }
    ]);
    expect(roundTripDocument.diagnostics).toEqual([]);
  });
});
