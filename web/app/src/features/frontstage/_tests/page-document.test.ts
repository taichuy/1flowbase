import { describe, expect, test } from 'vitest';

import {
  createFrontstageBlockRuntimeDescriptor,
  createFrontstagePageDocument,
  type FrontstageBlockInstance
} from '../lib/page-document';
import { createFrontstagePageContentFixture } from './frontstage-page-content-fixtures';

describe('frontstage page document', () => {
  test('AC-012 keeps only page and tab metadata and ignores legacy document blocks', () => {
    const document = createFrontstagePageDocument(
      createFrontstagePageContentFixture({
        document: {
          rootUid: 'root-1',
          payload: {
            version: 1,
            'x-layout-mode': 'free',
            blocks: [{ id: 'legacy-block', codeRef: 'legacy-code' }]
          }
        }
      })
    );

    expect(document.rootUid).toBe('root-1');
    expect(document.layoutMode).toBe('free');
    expect(document.blocks).toEqual([]);
    expect(document.isEmpty).toBe(true);
    expect(document.diagnostics).toEqual([]);
  });

  test('AC-008 serializes a Block Node descriptor without runtime-only source fields', () => {
    const block: FrontstageBlockInstance = {
      id: 'hero',
      rendererVersion: 'v1',
      sourceId: 'legacy-id',
      codeRef: 'hero-code',
      sourceCodeRef: 'legacy-code',
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
      ports: { inputs: [], outputs: [] },
      presentation: { heightMode: 'auto', height: null },
      layout: { region: 'main', order: 99, span: 12 },
      order: 3,
      runtime: { kind: 'inline', entry: null, hint: 'inline' }
    };

    expect(createFrontstageBlockRuntimeDescriptor(block)).toEqual({
      id: 'hero',
      renderer_version: 'v1',
      codeRef: 'hero-code',
      catalog: block.catalog,
      contribution: block.contribution,
      props: block.props,
      ports: block.ports,
      'x-presentation': block.presentation,
      'x-layout': { region: 'main', order: 3, span: 12 },
      runtime: block.runtime
    });
  });
});
