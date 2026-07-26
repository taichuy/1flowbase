import {
  nativeReactCatalogDependencyLockIdentity,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import {
  createRestrictedBlockRunPlan,
  type RestrictedBlockLoaderLimits,
  type RestrictedBlockLoaderRejection,
  type RestrictedBlockRunPlan
} from '../restricted-block-loader';
import type {
  FrontstagePageCanvasReadyRuntimeSource,
  FrontstagePageCanvasRuntimeSource,
  FrontstagePageCanvasRuntimeSourceState
} from './runtime-source';

export type FrontstagePageCanvasNativePreparationPlanItem =
  | {
      status: 'native_plan_ready';
      blockId: string;
      slotIndex: number;
      codeRef: string;
      sourceSha256: string;
      source: string;
      dependencyLock: NativeReactCatalogDependencyLock;
      dependencyLockIdentity: string;
    }
  | {
      status: 'source_not_ready';
      blockId: string;
      slotIndex: number;
      codeRef: string;
      sourceStatus: FrontstagePageCanvasRuntimeSource['status'];
    };

export interface FrontstagePageCanvasNativePreparationPlanState {
  workspaceId: string;
  pageId: string;
  items: FrontstagePageCanvasNativePreparationPlanItem[];
}

/** Native plan contains immutable preparation inputs only; instance payloads belong to P2. */
export function createFrontstagePageCanvasNativePreparationPlanState({
  sourceState,
  dependencyLocksByBlockId
}: {
  sourceState: FrontstagePageCanvasRuntimeSourceState;
  dependencyLocksByBlockId: Readonly<
    Record<string, NativeReactCatalogDependencyLock>
  >;
}): FrontstagePageCanvasNativePreparationPlanState {
  return {
    workspaceId: sourceState.workspaceId,
    pageId: sourceState.pageId,
    items: sourceState.sources.map((source) => {
      if (source.status === 'ready') {
        const dependencyLock = dependencyLocksByBlockId[source.blockId] ?? [];
        return {
          status: 'native_plan_ready',
          blockId: source.blockId,
          slotIndex: source.slotIndex,
          codeRef: source.codeRef,
          sourceSha256: source.source_sha256,
          source: source.code,
          dependencyLock,
          dependencyLockIdentity:
            nativeReactCatalogDependencyLockIdentity(dependencyLock)
        };
      }
      return {
        status: 'source_not_ready',
        blockId: source.blockId,
        slotIndex: source.slotIndex,
        codeRef: source.codeRef,
        sourceStatus: source.status
      };
    })
  };
}

export type FrontstagePageCanvasRuntimeRunPlanStatus =
  | 'run_plan_ready'
  | 'source_not_ready'
  | 'artifact_lookup_pending'
  | 'catalog_missing'
  | 'rejected';

export type FrontstagePageCanvasRuntimeRunPlanIssueCode =
  | 'source_not_ready'
  | 'artifact_lookup_pending'
  | 'catalog_missing';

export interface FrontstagePageCanvasRuntimeRunPlanIssue {
  code: FrontstagePageCanvasRuntimeRunPlanIssueCode;
  path: string;
  message: string;
}

export interface FrontstagePageCanvasRuntimeRunPlanItemBase {
  blockId: string;
  sourceBlockId: string | null;
  codeRef: string;
  sourceCodeRef: string | null;
  order: number;
  sourceIndex: number;
  slotIndex: number;
  renderMode: FrontstagePageCanvasRuntimeSource['renderMode'];
  canEnterRestrictedJsRuntime: boolean;
  runtimeKind: string;
  runtimeEntry: string | null;
  contributionCode: string;
  sourceStatus: FrontstagePageCanvasRuntimeSource['status'];
}

export interface FrontstagePageCanvasRuntimeRunPlanReadyItem extends FrontstagePageCanvasRuntimeRunPlanItemBase {
  status: 'run_plan_ready';
  sourceStatus: 'ready';
  source_sha256: string;
  catalogId: string;
  runPlan: RestrictedBlockRunPlan;
}

export interface FrontstagePageCanvasRuntimeRunPlanSourceNotReadyItem extends FrontstagePageCanvasRuntimeRunPlanItemBase {
  status: 'source_not_ready';
  reason: FrontstagePageCanvasRuntimeRunPlanIssue;
}

export interface FrontstagePageCanvasRuntimeRunPlanArtifactLookupPendingItem extends FrontstagePageCanvasRuntimeRunPlanItemBase {
  status: 'artifact_lookup_pending';
  sourceStatus: 'ready';
  reason: FrontstagePageCanvasRuntimeRunPlanIssue;
}

export interface FrontstagePageCanvasRuntimeRunPlanCatalogMissingItem extends FrontstagePageCanvasRuntimeRunPlanItemBase {
  status: 'catalog_missing';
  sourceStatus: 'ready';
  reason: FrontstagePageCanvasRuntimeRunPlanIssue;
}

export interface FrontstagePageCanvasRuntimeRunPlanRejectedItem extends FrontstagePageCanvasRuntimeRunPlanItemBase {
  status: 'rejected';
  sourceStatus: 'ready';
  catalogId: string;
  rejection: RestrictedBlockLoaderRejection;
}

export type FrontstagePageCanvasRuntimeRunPlanItem =
  | FrontstagePageCanvasRuntimeRunPlanReadyItem
  | FrontstagePageCanvasRuntimeRunPlanSourceNotReadyItem
  | FrontstagePageCanvasRuntimeRunPlanArtifactLookupPendingItem
  | FrontstagePageCanvasRuntimeRunPlanCatalogMissingItem
  | FrontstagePageCanvasRuntimeRunPlanRejectedItem;

export interface FrontstagePageCanvasRuntimeRunPlanState {
  workspaceId: string;
  pageId: string;
  items: FrontstagePageCanvasRuntimeRunPlanItem[];
}

export type FrontstagePageCanvasRuntimeRunPlanContextSnapshotInput =
  | Record<string, unknown>
  | ((
      source: FrontstagePageCanvasReadyRuntimeSource,
      sourceIndex: number
    ) => Record<string, unknown>);

export interface CreateFrontstagePageCanvasRuntimeRunPlanStateInput {
  sourceState: FrontstagePageCanvasRuntimeSourceState;
  catalogEntries: readonly NormalizedFrontstageBlockCatalogEntry[];
  contextSnapshot: FrontstagePageCanvasRuntimeRunPlanContextSnapshotInput;
  limits?: RestrictedBlockLoaderLimits;
}

export function createFrontstagePageCanvasRuntimeRunPlanState({
  sourceState,
  catalogEntries,
  contextSnapshot,
  limits
}: CreateFrontstagePageCanvasRuntimeRunPlanStateInput): FrontstagePageCanvasRuntimeRunPlanState {
  const catalogEntriesByMatchKey = createCatalogEntryMap(catalogEntries);

  return {
    workspaceId: sourceState.workspaceId,
    pageId: sourceState.pageId,
    items: sourceState.sources.map((source, sourceIndex) =>
      createRuntimeRunPlanItem({
        source,
        sourceIndex,
        catalogEntriesByMatchKey,
        contextSnapshot,
        limits
      })
    )
  };
}

function createRuntimeRunPlanItem({
  source,
  sourceIndex,
  catalogEntriesByMatchKey,
  contextSnapshot,
  limits
}: {
  source: FrontstagePageCanvasRuntimeSource;
  sourceIndex: number;
  catalogEntriesByMatchKey: ReadonlyMap<
    string,
    NormalizedFrontstageBlockCatalogEntry
  >;
  contextSnapshot: FrontstagePageCanvasRuntimeRunPlanContextSnapshotInput;
  limits?: RestrictedBlockLoaderLimits;
}): FrontstagePageCanvasRuntimeRunPlanItem {
  const base = createRunPlanItemBase(source);

  if (source.status !== 'ready') {
    return {
      ...base,
      status: 'source_not_ready',
      reason: createSourceNotReadyReason(source, sourceIndex)
    };
  }

  if (source.artifactLookupStatus === 'pending') {
    return {
      ...base,
      status: 'artifact_lookup_pending',
      sourceStatus: 'ready',
      reason: {
        code: 'artifact_lookup_pending',
        path: `sources.${sourceIndex}.artifactLookupStatus`,
        message: `Compiled artifact lookup is pending for block ${source.blockId}.`
      }
    };
  }

  const catalogEntry = findMatchingCatalogEntry(
    source,
    catalogEntriesByMatchKey
  );

  if (!catalogEntry) {
    return {
      ...base,
      status: 'catalog_missing',
      sourceStatus: 'ready',
      reason: createCatalogMissingReason(source)
    };
  }

  const runPlanResult = createRestrictedBlockRunPlan({
    block: source.block,
    catalogEntry,
    code: source.code,
    sourceSha256: source.source_sha256,
    compiledArtifact: source.compiledArtifact,
    contextSnapshot: resolveContextSnapshot(
      contextSnapshot,
      source,
      sourceIndex
    ),
    limits
  });

  if (!runPlanResult.ok) {
    return {
      ...base,
      status: 'rejected',
      sourceStatus: 'ready',
      catalogId: catalogEntry.id,
      rejection: runPlanResult
    };
  }

  return {
    ...base,
    status: 'run_plan_ready',
    sourceStatus: 'ready',
    source_sha256: source.source_sha256,
    catalogId: catalogEntry.id,
    runPlan: runPlanResult
  };
}

function createRunPlanItemBase(
  source: FrontstagePageCanvasRuntimeSource
): FrontstagePageCanvasRuntimeRunPlanItemBase {
  return {
    blockId: source.blockId,
    sourceBlockId: source.sourceBlockId,
    codeRef: source.codeRef,
    sourceCodeRef: source.sourceCodeRef,
    order: source.order,
    sourceIndex: source.sourceIndex,
    slotIndex: source.slotIndex,
    renderMode: source.renderMode,
    canEnterRestrictedJsRuntime: source.canEnterRestrictedJsRuntime,
    runtimeKind: source.runtimeKind,
    runtimeEntry: source.runtimeEntry,
    contributionCode: source.contributionCode,
    sourceStatus: source.status
  };
}

function createCatalogEntryMap(
  catalogEntries: readonly NormalizedFrontstageBlockCatalogEntry[]
): Map<string, NormalizedFrontstageBlockCatalogEntry> {
  const catalogEntriesByMatchKey = new Map<
    string,
    NormalizedFrontstageBlockCatalogEntry
  >();

  for (const catalogEntry of catalogEntries) {
    const matchKey = createCatalogEntryMatchKey(catalogEntry);

    if (matchKey && !catalogEntriesByMatchKey.has(matchKey)) {
      catalogEntriesByMatchKey.set(matchKey, catalogEntry);
    }
  }

  return catalogEntriesByMatchKey;
}

function findMatchingCatalogEntry(
  source: FrontstagePageCanvasReadyRuntimeSource,
  catalogEntriesByMatchKey: ReadonlyMap<
    string,
    NormalizedFrontstageBlockCatalogEntry
  >
): NormalizedFrontstageBlockCatalogEntry | null {
  const matchKey = createSourceMatchKey(source);

  return matchKey ? (catalogEntriesByMatchKey.get(matchKey) ?? null) : null;
}

function createCatalogEntryMatchKey(
  catalogEntry: NormalizedFrontstageBlockCatalogEntry
): string | null {
  return createMatchKey([
    catalogEntry.providerCode,
    catalogEntry.installationId,
    catalogEntry.pluginId,
    catalogEntry.pluginVersion,
    catalogEntry.contributionCode,
    catalogEntry.runtimeKind,
    catalogEntry.entry
  ]);
}

function createSourceMatchKey(
  source: FrontstagePageCanvasReadyRuntimeSource
): string | null {
  return createMatchKey([
    source.block.catalog.providerCode,
    source.block.catalog.installationId,
    source.block.contribution.pluginId,
    source.block.contribution.pluginVersion,
    source.block.contribution.code,
    source.block.runtime.kind,
    source.block.runtime.entry
  ]);
}

function createMatchKey(values: unknown[]): string | null {
  const parts: string[] = [];

  for (const value of values) {
    const normalizedValue = normalizeRequiredString(value);

    if (!normalizedValue) {
      return null;
    }

    parts.push(normalizedValue);
  }

  return JSON.stringify(parts);
}

function resolveContextSnapshot(
  contextSnapshot: FrontstagePageCanvasRuntimeRunPlanContextSnapshotInput,
  source: FrontstagePageCanvasReadyRuntimeSource,
  sourceIndex: number
): Record<string, unknown> {
  return typeof contextSnapshot === 'function'
    ? contextSnapshot(source, sourceIndex)
    : contextSnapshot;
}

function createSourceNotReadyReason(
  source: FrontstagePageCanvasRuntimeSource,
  sourceIndex: number
): FrontstagePageCanvasRuntimeRunPlanIssue {
  return {
    code: 'source_not_ready',
    path: `sources.${sourceIndex}.status`,
    message: `Runtime source for block ${source.blockId} is ${source.status}; no restricted run plan was created.`
  };
}

function createCatalogMissingReason(
  source: FrontstagePageCanvasReadyRuntimeSource
): FrontstagePageCanvasRuntimeRunPlanIssue {
  return {
    code: 'catalog_missing',
    path: 'catalogEntries',
    message: `No matching runtime catalog entry was found for block ${source.blockId}.`
  };
}

function normalizeRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}
