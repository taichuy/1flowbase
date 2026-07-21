import type {
  FrontstageBlockInstance,
  FrontstageBlockLayout,
  FrontstageBlockPresentation,
  FrontstagePageDocument,
  FrontstagePageLayoutMode
} from './page-document';
import { FRONTSTAGE_BLOCK_RENDERER_VERSION_V1 } from './block-renderer-version';

export interface FrontstageBlockCompositionState {
  document: FrontstagePageDocument;
  selectedBlockId: string | null;
}

export type FrontstageBlockCompositionInput = Partial<
  Omit<FrontstageBlockInstance, 'layout' | 'order'>
> & {
  layout?: Partial<FrontstageBlockLayout> & Record<string, unknown>;
  order?: number;
};

function asNonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0
    ? value.trim()
    : null;
}

function clampIndex(index: number, maxIndex: number): number {
  if (!Number.isFinite(index)) {
    return maxIndex;
  }

  if (index < 0) {
    return 0;
  }

  if (index > maxIndex) {
    return maxIndex;
  }

  return Math.trunc(index);
}

function toUniqueValue(preferredValue: string, usedValues: Set<string>): string {
  if (!usedValues.has(preferredValue)) {
    usedValues.add(preferredValue);
    return preferredValue;
  }

  let suffix = 2;
  let candidate = `${preferredValue}-${suffix}`;
  while (usedValues.has(candidate)) {
    suffix += 1;
    candidate = `${preferredValue}-${suffix}`;
  }

  usedValues.add(candidate);
  return candidate;
}

function sortBlocksByOrder(
  blocks: FrontstageBlockInstance[]
): FrontstageBlockInstance[] {
  return blocks
    .map((block, index) => ({ block, index }))
    .sort((left, right) => {
      const leftOrder = Number.isFinite(left.block.order)
        ? left.block.order
        : left.index;
      const rightOrder = Number.isFinite(right.block.order)
        ? right.block.order
        : right.index;

      if (leftOrder === rightOrder) {
        return left.index - right.index;
      }

      return leftOrder - rightOrder;
    })
    .map(({ block }) => block);
}

function cloneBlock(
  block: FrontstageBlockInstance,
  id: string,
  codeRef: string,
  order: number
): FrontstageBlockInstance {
  return {
    ...block,
    id,
    codeRef,
    catalog: { ...block.catalog },
    contribution: { ...block.contribution },
    props: { ...block.props },
    presentation: { ...block.presentation },
    layout: {
      ...block.layout,
      order
    },
    order,
    runtime: { ...block.runtime }
  };
}

function normalizeBlocks(
  blocks: FrontstageBlockInstance[],
  shouldSort: boolean
): FrontstageBlockInstance[] {
  const usedIds = new Set<string>();
  const usedCodeRefs = new Set<string>();
  const uniqueBlocks = blocks.map((block, index) => {
    const preferredId = asNonEmptyString(block.id) ?? `block-${index + 1}`;
    const id = toUniqueValue(preferredId, usedIds);
    const preferredCodeRef =
      asNonEmptyString(block.codeRef) ?? `${id}-code`;
    const codeRef = toUniqueValue(preferredCodeRef, usedCodeRefs);

    return cloneBlock(block, id, codeRef, block.order);
  });
  const orderedBlocks = shouldSort
    ? sortBlocksByOrder(uniqueBlocks)
    : uniqueBlocks;

  return orderedBlocks.map((block, index) =>
    cloneBlock(block, block.id, block.codeRef, index)
  );
}

function withBlocks(
  document: FrontstagePageDocument,
  blocks: FrontstageBlockInstance[],
  shouldSort: boolean
): FrontstagePageDocument {
  const normalizedBlocks = normalizeBlocks(blocks, shouldSort);

  return {
    ...document,
    blocks: normalizedBlocks,
    isEmpty: normalizedBlocks.length === 0,
    diagnostics: [...document.diagnostics]
  };
}

function hasBlock(document: FrontstagePageDocument, blockId: string | null) {
  return (
    blockId !== null && document.blocks.some((block) => block.id === blockId)
  );
}

function normalizeSelection(
  document: FrontstagePageDocument,
  selectedBlockId: string | null
): string | null {
  return hasBlock(document, selectedBlockId) ? selectedBlockId : null;
}

function createBlockFromInput(
  input: FrontstageBlockCompositionInput,
  index: number
): FrontstageBlockInstance {
  const id = asNonEmptyString(input.id) ?? `block-${index + 1}`;
  const codeRef = asNonEmptyString(input.codeRef) ?? `${id}-code`;
  const layoutOrder =
    typeof input.layout?.order === 'number' &&
    Number.isFinite(input.layout.order)
      ? input.layout.order
      : index;
  const order =
    typeof input.order === 'number' && Number.isFinite(input.order)
      ? input.order
      : layoutOrder;

  return {
    id,
    rendererVersion:
      input.rendererVersion ?? FRONTSTAGE_BLOCK_RENDERER_VERSION_V1,
    sourceId: input.sourceId ?? null,
    codeRef,
    sourceCodeRef: input.sourceCodeRef ?? null,
    catalog: {
      providerCode: input.catalog?.providerCode ?? null,
      installationId: input.catalog?.installationId ?? null
    },
    contribution: {
      pluginId: input.contribution?.pluginId ?? null,
      pluginVersion: input.contribution?.pluginVersion ?? null,
      code: input.contribution?.code ?? 'unknown'
    },
    props: { ...(input.props ?? {}) },
    presentation: {
      heightMode: input.presentation?.heightMode ?? 'auto',
      height:
        input.presentation?.heightMode === 'fixed'
          ? input.presentation.height ?? 320
          : null
    },
    layout: {
      ...(input.layout ?? {}),
      order
    },
    order,
    runtime: {
      kind: input.runtime?.kind ?? 'unknown',
      entry: input.runtime?.entry ?? null,
      hint: input.runtime?.hint ?? input.runtime?.kind ?? 'unknown',
      ...(input.runtime?.code_template_version
        ? { code_template_version: input.runtime.code_template_version }
        : {}),
      ...(input.runtime?.code_template_language
        ? { code_template_language: input.runtime.code_template_language }
        : {})
    }
  };
}

export function createFrontstageBlockCompositionState(
  document: FrontstagePageDocument,
  selectedBlockId: string | null = null
): FrontstageBlockCompositionState {
  const normalizedDocument = withBlocks(document, document.blocks, true);

  return {
    document: normalizedDocument,
    selectedBlockId: normalizeSelection(normalizedDocument, selectedBlockId)
  };
}

export function appendFrontstageBlock(
  state: FrontstageBlockCompositionState,
  input: FrontstageBlockCompositionInput
): FrontstageBlockCompositionState {
  return insertFrontstageBlock(state, state.document.blocks.length, input);
}

export function insertFrontstageBlock(
  state: FrontstageBlockCompositionState,
  index: number,
  input: FrontstageBlockCompositionInput
): FrontstageBlockCompositionState {
  const nextBlocks = [...state.document.blocks];
  const insertIndex = clampIndex(index, nextBlocks.length);
  nextBlocks.splice(insertIndex, 0, createBlockFromInput(input, insertIndex));

  const document = withBlocks(state.document, nextBlocks, false);
  const selectedBlockId = document.blocks[insertIndex]?.id ?? null;

  return {
    document,
    selectedBlockId
  };
}

export function removeFrontstageBlock(
  state: FrontstageBlockCompositionState,
  blockId: string
): FrontstageBlockCompositionState {
  const nextBlocks = state.document.blocks.filter(
    (block) => block.id !== blockId
  );
  const document = withBlocks(state.document, nextBlocks, false);

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function moveFrontstageBlock(
  state: FrontstageBlockCompositionState,
  blockId: string,
  toIndex: number
): FrontstageBlockCompositionState {
  const fromIndex = state.document.blocks.findIndex(
    (block) => block.id === blockId
  );
  if (fromIndex < 0) {
    return state;
  }

  const nextBlocks = [...state.document.blocks];
  const [block] = nextBlocks.splice(fromIndex, 1);
  nextBlocks.splice(clampIndex(toIndex, nextBlocks.length), 0, block);
  const document = withBlocks(state.document, nextBlocks, false);

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function updateFrontstageBlockLayout(
  state: FrontstageBlockCompositionState,
  blockId: string,
  layoutPatch: Partial<FrontstageBlockLayout> & Record<string, unknown>
): FrontstageBlockCompositionState {
  const targetIndex = state.document.blocks.findIndex(
    (block) => block.id === blockId
  );
  if (targetIndex < 0) {
    return state;
  }

  const nextBlocks = state.document.blocks.map((block, index) => {
    if (index !== targetIndex) {
      return block;
    }

    const nextLayout: FrontstageBlockLayout = {
      ...block.layout,
      ...layoutPatch,
      order: block.order
    };

    return {
      ...block,
      layout: nextLayout
    };
  });

  const document = withBlocks(state.document, nextBlocks, false);

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function updateFrontstagePageLayoutMode(
  state: FrontstageBlockCompositionState,
  layoutMode: FrontstagePageLayoutMode
): FrontstageBlockCompositionState {
  return {
    document: { ...state.document, layoutMode },
    selectedBlockId: state.selectedBlockId
  };
}

export function updateFrontstageBlock(
  state: FrontstageBlockCompositionState,
  nextBlock: FrontstageBlockInstance
): FrontstageBlockCompositionState {
  if (!state.document.blocks.some((block) => block.id === nextBlock.id)) {
    return state;
  }

  const document = withBlocks(
    state.document,
    state.document.blocks.map((block) =>
      block.id === nextBlock.id
        ? {
            ...block,
            props: { ...nextBlock.props },
            interfaces: (nextBlock.interfaces ?? []).map((binding) => ({
              ...binding
            })),
            ports: {
              inputs: (nextBlock.ports?.inputs ?? []).map((port) => ({
                ...port,
                schema: { ...port.schema },
                ...(port.source ? { source: { ...port.source } } : {})
              })),
              outputs: (nextBlock.ports?.outputs ?? []).map((port) => ({
                ...port,
                schema: { ...port.schema }
              }))
            },
            presentation: { ...nextBlock.presentation }
          }
        : block
    ),
    false
  );

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function updateFrontstageBlockProps(
  state: FrontstageBlockCompositionState,
  blockId: string,
  props: Record<string, unknown>
): FrontstageBlockCompositionState {
  if (!state.document.blocks.some((block) => block.id === blockId)) {
    return state;
  }

  const document = withBlocks(
    state.document,
    state.document.blocks.map((block) =>
      block.id === blockId
        ? {
            ...block,
            props: { ...props }
          }
        : block
    ),
    false
  );

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function updateFrontstageBlockPresentation(
  state: FrontstageBlockCompositionState,
  blockId: string,
  presentation: FrontstageBlockPresentation
): FrontstageBlockCompositionState {
  const targetIndex = state.document.blocks.findIndex(
    (block) => block.id === blockId
  );
  if (targetIndex < 0) {
    return state;
  }

  const nextBlocks = state.document.blocks.map((block, index) =>
    index === targetIndex
      ? { ...block, presentation: { ...presentation } }
      : block
  );
  const document = withBlocks(state.document, nextBlocks, false);

  return {
    document,
    selectedBlockId: normalizeSelection(document, state.selectedBlockId)
  };
}

export function selectFrontstageBlock(
  state: FrontstageBlockCompositionState,
  blockId: string | null
): FrontstageBlockCompositionState {
  return {
    document: state.document,
    selectedBlockId: normalizeSelection(state.document, blockId)
  };
}
