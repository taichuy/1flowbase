import { describe, expect, test } from 'vitest';

import {
  appendFrontstageBlock,
  createFrontstageBlockCompositionState,
  insertFrontstageBlock,
  moveFrontstageBlock,
  removeFrontstageBlock,
  selectFrontstageBlock,
  updateFrontstageBlock,
  updateFrontstageBlockLayout,
  updateFrontstageBlockProps
} from '../lib/block-composition';
import type {
  FrontstageBlockInstance,
  FrontstagePageDocument
} from '../lib/page-document';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  const id = overrides.id ?? 'block-1';
  const codeRef = overrides.codeRef ?? `${id}-code`;

  return {
    id,
    rendererVersion: overrides.rendererVersion ?? 'v1',
    sourceId: overrides.sourceId ?? id,
    codeRef,
    sourceCodeRef: overrides.sourceCodeRef ?? codeRef,
    catalog: overrides.catalog ?? {
      providerCode: null,
      installationId: null
    },
    contribution: overrides.contribution ?? {
      pluginId: null,
      pluginVersion: null,
      code: 'unknown'
    },
    props: overrides.props ?? {},
    presentation: overrides.presentation ?? { heightMode: 'auto', height: null },
    layout: overrides.layout ?? { order: overrides.order ?? 0 },
    order: overrides.order ?? 0,
    runtime: overrides.runtime ?? {
      kind: 'unknown',
      entry: null,
      hint: 'unknown'
    }
  };
}

function createDocument(
  blocks: FrontstageBlockInstance[] = []
): FrontstagePageDocument {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page',
      parentId: null,
      rank: '001000',
      contentPresentation: 'single',
    },
    rootUid: 'root-1',
    layoutMode: 'auto',
    blocks,
    isEmpty: blocks.length === 0,
    diagnostics: []
  };
}

describe('frontstage block composition', () => {
  test('creates a normalized composition state with stable order and unique refs', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([
        createBlock({ id: 'hero', codeRef: 'hero-code', order: 30 }),
        createBlock({ id: 'hero', codeRef: 'hero-code', order: 10 })
      ]),
      'hero-2'
    );

    expect(state.document.blocks.map((block) => block.id)).toEqual([
      'hero-2',
      'hero'
    ]);
    expect(state.document.blocks.map((block) => block.codeRef)).toEqual([
      'hero-code-2',
      'hero-code'
    ]);
    expect(state.document.blocks.map((block) => block.order)).toEqual([0, 1]);
    expect(state.document.blocks.map((block) => block.layout.order)).toEqual([
      0,
      1
    ]);
    expect(state.document.isEmpty).toBe(false);
    expect(state.selectedBlockId).toBe('hero-2');
  });

  test('appends and inserts blocks with generated ids and selected state', () => {
    const initialState = createFrontstageBlockCompositionState(
      createDocument([createBlock({ id: 'hero', codeRef: 'hero-code' })])
    );

    const appended = appendFrontstageBlock(initialState, {
      id: 'hero',
      codeRef: 'hero-code',
      contribution: { pluginId: null, pluginVersion: null, code: 'banner' },
      runtime: { kind: 'inline', entry: null, hint: 'inline' }
    });
    const inserted = insertFrontstageBlock(appended, 1, {
      id: 'cta',
      codeRef: 'cta-code'
    });

    expect(inserted.document.blocks.map((block) => block.id)).toEqual([
      'hero',
      'cta',
      'hero-2'
    ]);
    expect(inserted.document.blocks.map((block) => block.codeRef)).toEqual([
      'hero-code',
      'cta-code',
      'hero-code-2'
    ]);
    expect(inserted.document.blocks.map((block) => block.order)).toEqual([
      0,
      1,
      2
    ]);
    expect(
      inserted.document.blocks.map((block) => block.rendererVersion)
    ).toEqual(['v1', 'v1', 'v1']);
    expect(inserted.selectedBlockId).toBe('cta');
  });

  test('removes blocks and clears an invalid selection', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([
        createBlock({ id: 'hero', order: 0 }),
        createBlock({ id: 'cta', order: 1 })
      ]),
      'cta'
    );

    const nextState = removeFrontstageBlock(state, 'cta');

    expect(nextState.document.blocks.map((block) => block.id)).toEqual([
      'hero'
    ]);
    expect(nextState.document.blocks[0].order).toBe(0);
    expect(nextState.document.isEmpty).toBe(false);
    expect(nextState.selectedBlockId).toBeNull();
  });

  test('moves blocks by id and clamps target indexes', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([
        createBlock({ id: 'hero', order: 0 }),
        createBlock({ id: 'gallery', order: 1 }),
        createBlock({ id: 'cta', order: 2 })
      ]),
      'gallery'
    );

    const movedToEnd = moveFrontstageBlock(state, 'hero', 99);
    const movedToStart = moveFrontstageBlock(movedToEnd, 'cta', -10);

    expect(movedToStart.document.blocks.map((block) => block.id)).toEqual([
      'cta',
      'gallery',
      'hero'
    ]);
    expect(movedToStart.document.blocks.map((block) => block.order)).toEqual([
      0,
      1,
      2
    ]);
    expect(movedToStart.selectedBlockId).toBe('gallery');
  });

  test('updates a block layout without changing order or selection', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([
        createBlock({
          id: 'hero',
          order: 0,
          layout: { order: 0, region: 'main', width: 12, height: 4 }
        }),
        createBlock({ id: 'cta', order: 1 })
      ]),
      'hero'
    );

    const nextState = updateFrontstageBlockLayout(state, 'hero', {
      width: 16
    });

    expect(nextState.document.blocks[0].layout).toMatchObject({
      order: 0,
      region: 'main',
      width: 16,
      height: 4
    });
    expect(nextState.document.blocks.map((block) => block.order)).toEqual([
      0,
      1
    ]);
    expect(nextState.selectedBlockId).toBe('hero');
  });

  test('updates structured block props without changing layout or selection', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([
        createBlock({
          id: 'hero',
          props: { title: 'Old title' },
          layout: { order: 0, width: 12 }
        })
      ]),
      'hero'
    );

    const nextState = updateFrontstageBlockProps(state, 'hero', {
      title: 'New title',
      dataBinding: []
    });

    expect(nextState.document.blocks[0]).toMatchObject({
      props: { title: 'New title', dataBinding: [] },
      layout: { order: 0, width: 12 }
    });
    expect(nextState.selectedBlockId).toBe('hero');
  });

  test('updates the complete studio-owned block contract without changing host order or selection', () => {
    const currentBlock = createBlock({
      id: 'hero',
      props: { title: 'Old title' },
      interfaces: [],
      ports: { inputs: [], outputs: [] },
      presentation: { heightMode: 'auto', height: null },
      layout: { order: 0, region: 'main', width: 12 },
      order: 0
    });
    const state = createFrontstageBlockCompositionState(
      createDocument([
        currentBlock,
        createBlock({ id: 'cta', order: 1 })
      ]),
      'hero'
    );

    const nextState = updateFrontstageBlock(state, {
      ...currentBlock,
      props: { title: 'New title' },
      interfaces: [
        {
          alias: 'getFrontstagePageDetail',
          operation_id: 'get_frontstage_page_detail',
          schema_digest: 'sha256:test',
          scope: 'frontstage',
          risk_level: 'read'
        }
      ],
      ports: {
        inputs: [],
        outputs: [{ name: 'total', schema: { type: 'number' } }]
      },
      presentation: { heightMode: 'fixed', height: 420 },
      layout: { order: 99, region: 'sidebar', width: 4 },
      order: 99
    });

    expect(nextState.document.blocks[0]).toMatchObject({
      id: 'hero',
      props: { title: 'New title' },
      interfaces: [
        {
          alias: 'getFrontstagePageDetail',
          operation_id: 'get_frontstage_page_detail'
        }
      ],
      ports: {
        inputs: [],
        outputs: [{ name: 'total', schema: { type: 'number' } }]
      },
      presentation: { heightMode: 'fixed', height: 420 },
      layout: { order: 0, region: 'main', width: 12 },
      order: 0
    });
    expect(nextState.document.blocks.map((block) => block.id)).toEqual([
      'hero',
      'cta'
    ]);
    expect(nextState.selectedBlockId).toBe('hero');
  });

  test('selects only existing blocks', () => {
    const state = createFrontstageBlockCompositionState(
      createDocument([createBlock({ id: 'hero' })])
    );

    expect(selectFrontstageBlock(state, 'hero').selectedBlockId).toBe('hero');
    expect(selectFrontstageBlock(state, 'missing').selectedBlockId).toBeNull();
    expect(selectFrontstageBlock(state, null).selectedBlockId).toBeNull();
  });
});
