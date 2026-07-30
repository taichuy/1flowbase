import {
  FRONTSTAGE_BLOCK_RUNTIME_KINDS,
  isFrontstageBlockNativeRuntime,
  type FrontstageBlockRuntimeKind
} from '../block-catalog';
import { isSupportedFrontstageBlockRendererVersion } from '../block-renderer-version';
import type {
  FrontstageBlockCatalogRef,
  FrontstageBlockContributionRef,
  FrontstageBlockInstance,
  FrontstageBlockLayout,
  FrontstageBlockPresentation,
  FrontstageBlockRuntimeHint,
  FrontstagePageDocument,
  FrontstagePageDocumentDiagnostic
} from '../page-document';

export type FrontstagePageRenderMode = 'native_react' | 'placeholder';

export type FrontstagePageRenderPlanFallbackReasonCode =
  | 'missing_code_ref'
  | 'missing_renderer_version'
  | 'missing_runtime_entry'
  | 'unknown_runtime'
  | 'unsupported_renderer_version'
  | 'unsupported_runtime';

export interface FrontstagePageRenderPlanFallbackReason {
  code: FrontstagePageRenderPlanFallbackReasonCode;
  path: string;
  message: string;
}

export interface FrontstageBlockRenderPlanItem {
  blockId: string;
  sourceBlockId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  rendererVersion: string | null;
  sourceIndex: number;
  order: number;
  renderMode: FrontstagePageRenderMode;
  canPrepareNativeReact: boolean;
  fallbackReasons: FrontstagePageRenderPlanFallbackReason[];
  catalog: FrontstageBlockCatalogRef;
  contribution: FrontstageBlockContributionRef;
  runtime: FrontstageBlockRuntimeHint;
  presentation: FrontstageBlockPresentation;
  layout: FrontstageBlockLayout;
  props: Record<string, unknown>;
}

export interface FrontstagePageRenderPlan {
  pageId: string;
  rootUid: string;
  isEmpty: boolean;
  diagnostics: FrontstagePageDocumentDiagnostic[];
  items: FrontstageBlockRenderPlanItem[];
}

const knownRuntimeKinds = new Set<string>(FRONTSTAGE_BLOCK_RUNTIME_KINDS);

function asRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => cloneValue(item)) as T;
  }

  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, cloneValue(entry)])
    ) as T;
  }

  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function cloneCatalog(
  catalog: FrontstageBlockCatalogRef
): FrontstageBlockCatalogRef {
  return { ...catalog };
}

function cloneContribution(
  contribution: FrontstageBlockContributionRef
): FrontstageBlockContributionRef {
  return { ...contribution };
}

function cloneRuntime(
  runtime: FrontstageBlockRuntimeHint
): FrontstageBlockRuntimeHint {
  return { ...runtime };
}

function cloneLayout(layout: FrontstageBlockLayout): FrontstageBlockLayout {
  return cloneValue(layout);
}

function cloneProps(props: Record<string, unknown>): Record<string, unknown> {
  return cloneValue(props);
}

function cloneDiagnostic(
  diagnostic: FrontstagePageDocumentDiagnostic
): FrontstagePageDocumentDiagnostic {
  return { ...diagnostic };
}

function createMissingCodeRefReason(
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'missing_code_ref',
    path: `blocks.${sourceIndex}.codeRef`,
    message:
      'Frontstage block cannot enter the Native React runtime without an original codeRef.'
  };
}

function createMissingRuntimeEntryReason(
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'missing_runtime_entry',
    path: `blocks.${sourceIndex}.runtime.entry`,
    message:
      'Frontstage block cannot enter the Native React runtime without a runtime entry.'
  };
}

function createMissingRendererVersionReason(
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'missing_renderer_version',
    path: `blocks.${sourceIndex}.renderer_version`,
    message:
      'Frontstage block cannot enter the runtime without a renderer_version.'
  };
}

function createUnsupportedRendererVersionReason(
  sourceIndex: number,
  rendererVersion: string
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'unsupported_renderer_version',
    path: `blocks.${sourceIndex}.renderer_version`,
    message: `Frontstage block renderer version "${rendererVersion}" is not supported by this client.`
  };
}

function createUnknownRuntimeReason(
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'unknown_runtime',
    path: `blocks.${sourceIndex}.runtime.kind`,
    message:
      'Frontstage block runtime is unknown and will render as a placeholder.'
  };
}

function createUnsupportedRuntimeReason(
  sourceIndex: number,
  runtimeKind: string
): FrontstagePageRenderPlanFallbackReason {
  return {
    code: 'unsupported_runtime',
    path: `blocks.${sourceIndex}.runtime.kind`,
    message: `Frontstage block runtime "${runtimeKind}" is not supported by the Native React runtime.`
  };
}

function resolveRuntimeReason(
  block: FrontstageBlockInstance,
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason | null {
  const runtimeKind = asRequiredString(block.runtime.kind);

  if (!runtimeKind || runtimeKind === 'unknown') {
    return createUnknownRuntimeReason(sourceIndex);
  }

  if (
    !knownRuntimeKinds.has(runtimeKind) ||
    !isFrontstageBlockNativeRuntime(runtimeKind as FrontstageBlockRuntimeKind)
  ) {
    return createUnsupportedRuntimeReason(sourceIndex, runtimeKind);
  }

  return null;
}

function resolveRendererVersionReason(
  block: FrontstageBlockInstance,
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason | null {
  const rendererVersion = asRequiredString(block.rendererVersion);

  if (!rendererVersion) {
    return createMissingRendererVersionReason(sourceIndex);
  }

  if (!isSupportedFrontstageBlockRendererVersion(rendererVersion)) {
    return createUnsupportedRendererVersionReason(sourceIndex, rendererVersion);
  }

  return null;
}

function createFallbackReasons(
  block: FrontstageBlockInstance,
  sourceIndex: number
): FrontstagePageRenderPlanFallbackReason[] {
  const reasons: FrontstagePageRenderPlanFallbackReason[] = [];

  const rendererVersionReason = resolveRendererVersionReason(
    block,
    sourceIndex
  );
  if (rendererVersionReason) {
    reasons.push(rendererVersionReason);
  }

  if (
    !asRequiredString(block.codeRef) ||
    !asRequiredString(block.sourceCodeRef)
  ) {
    reasons.push(createMissingCodeRefReason(sourceIndex));
  }

  const runtimeReason = resolveRuntimeReason(block, sourceIndex);
  if (runtimeReason) {
    reasons.push(runtimeReason);
  }

  if (!asRequiredString(block.runtime.entry)) {
    reasons.push(createMissingRuntimeEntryReason(sourceIndex));
  }

  return reasons;
}

function compareRenderPlanItems(
  left: FrontstageBlockRenderPlanItem,
  right: FrontstageBlockRenderPlanItem
): number {
  const leftOrder = Number.isFinite(left.order) ? left.order : left.sourceIndex;
  const rightOrder = Number.isFinite(right.order)
    ? right.order
    : right.sourceIndex;

  if (leftOrder === rightOrder) {
    return left.sourceIndex - right.sourceIndex;
  }

  return leftOrder - rightOrder;
}

export function createFrontstageBlockRenderPlanItem(
  block: FrontstageBlockInstance,
  sourceIndex = 0
): FrontstageBlockRenderPlanItem {
  const fallbackReasons = createFallbackReasons(block, sourceIndex);
  const canPrepareNativeReact = fallbackReasons.length === 0;

  return {
    blockId: block.id,
    sourceBlockId: block.sourceId,
    codeRef: block.codeRef,
    sourceCodeRef: block.sourceCodeRef,
    rendererVersion: block.rendererVersion,
    sourceIndex,
    order: block.order,
    renderMode: canPrepareNativeReact ? 'native_react' : 'placeholder',
    canPrepareNativeReact,
    fallbackReasons,
    catalog: cloneCatalog(block.catalog),
    contribution: cloneContribution(block.contribution),
    runtime: cloneRuntime(block.runtime),
    presentation: { ...block.presentation },
    layout: cloneLayout(block.layout),
    props: cloneProps(block.props)
  };
}

export function createFrontstagePageRenderPlan(
  document: FrontstagePageDocument
): FrontstagePageRenderPlan {
  const items = document.blocks
    .map((block, sourceIndex) =>
      createFrontstageBlockRenderPlanItem(block, sourceIndex)
    )
    .sort(compareRenderPlanItems);

  return {
    pageId: document.page.id,
    rootUid: document.rootUid,
    isEmpty: items.length === 0,
    diagnostics: document.diagnostics.map(cloneDiagnostic),
    items
  };
}
