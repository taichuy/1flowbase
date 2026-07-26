import { sha256Text } from '@1flowbase/page-runtime';

import type {
  FrontstageBlockRenderPlanItem,
  FrontstagePageRenderPlan
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

export function frontstageRuntimeSourceMatchesDigest(
  code: string,
  sourceSha256: string
): boolean {
  return sha256Text(code) === sourceSha256.toLowerCase();
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
    slot.renderMode !== 'native_react' ||
    !slot.canPrepareNativeReact ||
    slot.fallbackReasons.length > 0 ||
    !codeRef ||
    !rendererVersion ||
    !runtimeEntry
  ) {
    return null;
  }

  const request = {
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
    requestId: [
      'frontstage-page-canvas-block-code',
      workspaceId,
      pageId,
      String(slotIndex),
      slot.blockId,
      codeRef
    ].join(':'),
    ...request
  };
}

function normalizeRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0
    ? value.trim()
    : null;
}
