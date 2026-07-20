import type {
  FrontstageBlockCatalogRef,
  FrontstageBlockContributionRef,
  FrontstageBlockInstance,
  FrontstageBlockLayout,
  FrontstageBlockRuntimeHint
} from '../page-document';
import type {
  FrontstageBlockRenderPlanItem,
  FrontstagePageRenderPlan,
  FrontstagePageRenderPlanFallbackReason,
  FrontstagePageRenderMode
} from './render-plan';

export interface FrontstagePageCanvasBlockCodeReadRequest {
  requestId: string;
  workspaceId: string;
  pageId: string;
  blockId: string;
  sourceBlockId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  runtimeEntry: string;
  runtimeKind: string;
  order: number;
  sourceIndex: number;
  slotIndex: number;
  contributionCode: string;
}

export interface FrontstagePageCanvasBlockCodeReadPlan {
  workspaceId: string;
  pageId: string;
  requests: FrontstagePageCanvasBlockCodeReadRequest[];
}

export type FrontstagePageCanvasBlockCodeReadResult =
  | {
      codeRef: string;
      status: 'ready';
      code: string;
    }
  | {
      codeRef: string;
      status: 'dormant';
    }
  | {
      codeRef: string;
      status: 'loading';
    }
  | {
      codeRef: string;
      status: 'missing';
      message?: string;
    }
  | {
      codeRef: string;
      status: 'failed';
      error?: unknown;
      message?: string;
    };

export interface FrontstagePageCanvasRuntimeSourceError {
  name?: string;
  code?: string;
  message: string;
}

interface FrontstagePageCanvasRuntimeSourceBase {
  blockId: string;
  sourceBlockId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  order: number;
  sourceIndex: number;
  slotIndex: number;
  renderMode: FrontstagePageRenderMode;
  canEnterRestrictedJsRuntime: boolean;
  runtimeKind: string;
  runtimeEntry: string | null;
  contributionCode: string;
}

export interface FrontstagePageCanvasReadyRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'ready';
  code: string;
  block: FrontstageBlockInstance;
  request: FrontstagePageCanvasBlockCodeReadRequest;
}

export interface FrontstagePageCanvasLoadingRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'loading';
  request: FrontstagePageCanvasBlockCodeReadRequest;
}

export interface FrontstagePageCanvasDormantRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'dormant';
  request: FrontstagePageCanvasBlockCodeReadRequest;
}

export interface FrontstagePageCanvasMissingRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'missing';
  message: string;
  request: FrontstagePageCanvasBlockCodeReadRequest;
}

export interface FrontstagePageCanvasFailedRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'failed';
  error: FrontstagePageCanvasRuntimeSourceError;
  request: FrontstagePageCanvasBlockCodeReadRequest;
}

export interface FrontstagePageCanvasSkippedRuntimeSource extends FrontstagePageCanvasRuntimeSourceBase {
  status: 'skipped';
  fallbackReasons: FrontstagePageRenderPlanFallbackReason[];
}

export type FrontstagePageCanvasRuntimeSource =
  | FrontstagePageCanvasReadyRuntimeSource
  | FrontstagePageCanvasDormantRuntimeSource
  | FrontstagePageCanvasLoadingRuntimeSource
  | FrontstagePageCanvasMissingRuntimeSource
  | FrontstagePageCanvasFailedRuntimeSource
  | FrontstagePageCanvasSkippedRuntimeSource;

export interface FrontstagePageCanvasRuntimeSourceState {
  workspaceId: string;
  pageId: string;
  sources: FrontstagePageCanvasRuntimeSource[];
}

export function createFrontstagePageCanvasBlockCodeReadPlan({
  workspaceId,
  renderPlan
}: {
  workspaceId: string;
  renderPlan: FrontstagePageRenderPlan;
}): FrontstagePageCanvasBlockCodeReadPlan {
  return {
    workspaceId,
    pageId: renderPlan.pageId,
    requests: renderPlan.items.flatMap((slot, slotIndex) => {
      const request = createReadRequest(
        slot,
        slotIndex,
        workspaceId,
        renderPlan.pageId
      );

      return request ? [request] : [];
    })
  };
}

export function createFrontstagePageCanvasRuntimeSourceState({
  renderPlan,
  readPlan,
  codeResults
}: {
  renderPlan: FrontstagePageRenderPlan;
  readPlan: FrontstagePageCanvasBlockCodeReadPlan;
  codeResults: FrontstagePageCanvasBlockCodeReadResult[];
}): FrontstagePageCanvasRuntimeSourceState {
  const resultsByCodeRef = createCodeResultMap(codeResults);
  const requestsBySlot = createReadRequestMap(readPlan.requests);

  return {
    workspaceId: readPlan.workspaceId,
    pageId: renderPlan.pageId,
    sources: renderPlan.items.map((slot, slotIndex) => {
      const request =
        requestsBySlot.get(createSlotKey(slot, slotIndex)) ??
        createReadRequest(
          slot,
          slotIndex,
          readPlan.workspaceId,
          readPlan.pageId
        );

      if (!request) {
        return {
          ...createRuntimeSourceBase(slot, slotIndex),
          status: 'skipped',
          fallbackReasons: cloneFallbackReasons(slot.fallbackReasons)
        };
      }

      const result = resultsByCodeRef.get(request.codeRef);
      if (result?.status === 'dormant') {
        return {
          ...createRuntimeSourceBase(slot, slotIndex),
          status: 'dormant',
          request: cloneReadRequest(request)
        };
      }

      if (!result || result.status === 'loading') {
        return {
          ...createRuntimeSourceBase(slot, slotIndex),
          status: 'loading',
          request: cloneReadRequest(request)
        };
      }

      if (result.status === 'missing') {
        return {
          ...createRuntimeSourceBase(slot, slotIndex),
          status: 'missing',
          message:
            normalizeOptionalString(result.message) ??
            `Block code is missing for ${request.codeRef}.`,
          request: cloneReadRequest(request)
        };
      }

      if (result.status === 'failed') {
        return {
          ...createRuntimeSourceBase(slot, slotIndex),
          status: 'failed',
          error: summarizeReadError(result),
          request: cloneReadRequest(request)
        };
      }

      return {
        ...createRuntimeSourceBase(slot, slotIndex),
        status: 'ready',
        code: result.code,
        block: createBlockFromSlot(slot),
        request: cloneReadRequest(request)
      };
    })
  };
}

function createReadRequest(
  slot: FrontstageBlockRenderPlanItem,
  slotIndex: number,
  workspaceId: string,
  pageId: string
): FrontstagePageCanvasBlockCodeReadRequest | null {
  const codeRef = normalizeRequiredString(slot.codeRef);
  const rendererVersion = normalizeRequiredString(slot.rendererVersion);
  const runtimeEntry = normalizeRequiredString(slot.runtime.entry);

  if (
    slot.renderMode !== 'restricted_js_block' ||
    !slot.canEnterRestrictedJsRuntime ||
    slot.fallbackReasons.length > 0 ||
    !codeRef ||
    !rendererVersion ||
    !runtimeEntry
  ) {
    return null;
  }

  const requestBase = {
    workspaceId,
    pageId,
    blockId: slot.blockId,
    sourceBlockId: slot.sourceBlockId,
    codeRef,
    sourceCodeRef: slot.sourceCodeRef,
    runtimeEntry,
    runtimeKind: slot.runtime.kind,
    order: slot.order,
    sourceIndex: slot.sourceIndex,
    slotIndex,
    contributionCode: slot.contribution.code
  };

  return {
    requestId: createRequestId(requestBase),
    ...requestBase
  };
}

function createRequestId(
  request: Omit<FrontstagePageCanvasBlockCodeReadRequest, 'requestId'>
): string {
  return [
    'frontstage-page-canvas-block-code',
    request.workspaceId,
    request.pageId,
    String(request.slotIndex),
    request.blockId,
    request.codeRef
  ].join(':');
}

function createCodeResultMap(
  codeResults: FrontstagePageCanvasBlockCodeReadResult[]
): Map<string, FrontstagePageCanvasBlockCodeReadResult> {
  const resultMap = new Map<string, FrontstagePageCanvasBlockCodeReadResult>();

  for (const result of codeResults) {
    const codeRef = normalizeRequiredString(result.codeRef);
    if (codeRef) {
      resultMap.set(codeRef, result);
    }
  }

  return resultMap;
}

function createReadRequestMap(
  requests: FrontstagePageCanvasBlockCodeReadRequest[]
): Map<string, FrontstagePageCanvasBlockCodeReadRequest> {
  const requestMap = new Map<
    string,
    FrontstagePageCanvasBlockCodeReadRequest
  >();

  for (const request of requests) {
    requestMap.set(createRequestSlotKey(request), request);
  }

  return requestMap;
}

function createRequestSlotKey(
  request: FrontstagePageCanvasBlockCodeReadRequest
): string {
  return [
    request.slotIndex,
    request.sourceIndex,
    request.blockId,
    request.codeRef
  ].join(':');
}

function createSlotKey(
  slot: FrontstageBlockRenderPlanItem,
  slotIndex: number
): string {
  return [slotIndex, slot.sourceIndex, slot.blockId, slot.codeRef].join(':');
}

function createRuntimeSourceBase(
  slot: FrontstageBlockRenderPlanItem,
  slotIndex: number
): FrontstagePageCanvasRuntimeSourceBase {
  return {
    blockId: slot.blockId,
    sourceBlockId: slot.sourceBlockId,
    codeRef: slot.codeRef,
    sourceCodeRef: slot.sourceCodeRef,
    order: slot.order,
    sourceIndex: slot.sourceIndex,
    slotIndex,
    renderMode: slot.renderMode,
    canEnterRestrictedJsRuntime: slot.canEnterRestrictedJsRuntime,
    runtimeKind: slot.runtime.kind,
    runtimeEntry: slot.runtime.entry,
    contributionCode: slot.contribution.code
  };
}

function createBlockFromSlot(
  slot: FrontstageBlockRenderPlanItem
): FrontstageBlockInstance {
  return {
    id: slot.blockId,
    rendererVersion: slot.rendererVersion,
    sourceId: slot.sourceBlockId,
    codeRef: slot.codeRef,
    sourceCodeRef: slot.sourceCodeRef,
    catalog: cloneCatalog(slot.catalog),
    contribution: cloneContribution(slot.contribution),
    props: cloneProps(slot.props),
    presentation: { ...slot.presentation },
    layout: cloneLayout(slot.layout),
    order: slot.order,
    runtime: cloneRuntime(slot.runtime)
  };
}

function cloneReadRequest(
  request: FrontstagePageCanvasBlockCodeReadRequest
): FrontstagePageCanvasBlockCodeReadRequest {
  return { ...request };
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

function cloneFallbackReasons(
  reasons: FrontstagePageRenderPlanFallbackReason[]
): FrontstagePageRenderPlanFallbackReason[] {
  return reasons.map((reason) => ({ ...reason }));
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

function summarizeReadError(
  result: Extract<FrontstagePageCanvasBlockCodeReadResult, { status: 'failed' }>
): FrontstagePageCanvasRuntimeSourceError {
  const errorSummary = summarizeUnknownError(result.error);
  const message = normalizeOptionalString(result.message);

  return {
    ...errorSummary,
    message:
      message ??
      errorSummary.message ??
      `Block code read failed for ${result.codeRef}.`
  };
}

function summarizeUnknownError(
  error: unknown
): Partial<FrontstagePageCanvasRuntimeSourceError> {
  if (error instanceof Error) {
    return {
      name: normalizeOptionalString(error.name),
      message: normalizeOptionalString(error.message)
    };
  }

  if (typeof error === 'string') {
    return {
      message: normalizeOptionalString(error)
    };
  }

  if (isRecord(error)) {
    return {
      name: getOptionalStringField(error, 'name'),
      code: getOptionalStringField(error, 'code'),
      message: getOptionalStringField(error, 'message')
    };
  }

  return {};
}

function getOptionalStringField(
  source: Record<string, unknown>,
  field: string
): string | undefined {
  return normalizeOptionalString(source[field]);
}

function normalizeRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function normalizeOptionalString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
