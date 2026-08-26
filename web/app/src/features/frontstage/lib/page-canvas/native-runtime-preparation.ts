import {
  type NativeReactArtifactEvaluationResult,
  type NativeReactComponentArtifact,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';

import { i18nText } from '../../../../shared/i18n/text';

import {
  createFrontstageRuntimeDemandCandidates,
  resolveFrontstageRuntimePreparationKind,
  type FrontstageRuntimeDemandByBlockId,
  type FrontstageRuntimeDemandPriority
} from './runtime-demand';
import type {
  FrontstageNativeRuntimeObservationStage,
  FrontstageRuntimeObservationCacheTier,
  FrontstageRuntimeObservationContext
} from './runtime-observation';
import type { PreparedTrustedFrontendContribution } from '../native-trusted-block-contribution-lifecycle';
import { discoverTrustedFrontendContribution } from '../native-trusted-block-contribution-lifecycle';
import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import type { FrontstagePageCanvasBlockCodeReadRequest } from './runtime-source';

export type FrontstageNativePreparationStage =
  | 'idle'
  | 'source_fetch'
  | 'artifact_lookup'
  | 'compile'
  | 'module_resolve'
  | 'ready'
  | 'failed'
  | 'disposed';

export type FrontstageNativePreparationActiveStage = Exclude<
  FrontstageNativePreparationStage,
  'idle' | 'ready' | 'failed' | 'disposed'
>;

export interface FrontstageNativeInstanceIdentityInput {
  sourceSha256: string;
  compilerAbi: string;
  runtimeAbi: string;
}

export interface FrontstageNativeInstanceMountIntent {
  blockId: string;
  slotIndex: number;
  identityInput: FrontstageNativeInstanceIdentityInput;
}

export interface FrontstageNativePreparedRuntime {
  artifact: NativeReactComponentArtifact;
  component: Extract<
    NativeReactArtifactEvaluationResult,
    { ok: true }
  >['component'];
  identityInput: FrontstageNativeInstanceIdentityInput;
  artifactCacheTier: 'l2' | 'miss';
  moduleAssets: NativeReactResolvedModuleAsset[];
  contribution?: PreparedTrustedFrontendContribution;
}

export function prepareFrontstageNativeContribution(
  catalogEntries: readonly NormalizedFrontstageBlockCatalogEntry[],
  request: FrontstagePageCanvasBlockCodeReadRequest,
  workspaceId: string
): PreparedTrustedFrontendContribution {
  const catalogEntry = catalogEntries.find(
    (entry) =>
      entry.installationId === request.installationId &&
      entry.providerCode === request.providerCode &&
      entry.pluginId === request.pluginId &&
      entry.pluginVersion === request.pluginVersion &&
      entry.contributionCode === request.contributionCode
  );
  if (!catalogEntry) {
    throw new Error(i18nText('frontstage', 'auto.runtime_preview_unavailable'));
  }
  return discoverTrustedFrontendContribution(catalogEntry.raw, {
    workspaceId,
    installationId: request.installationId ?? '',
    providerCode: request.providerCode ?? '',
    pluginId: request.pluginId ?? '',
    pluginVersion: request.pluginVersion ?? '',
    contributionCode: request.contributionCode
  }).prepare();
}

interface FrontstageNativePreparationSnapshotBase {
  blockId: string;
  slotIndex: number;
  priority: FrontstageRuntimeDemandPriority;
  generation: number;
  observationContext?: FrontstageRuntimeObservationContext;
}

export type FrontstageNativePreparationSnapshot =
  | (FrontstageNativePreparationSnapshotBase & { status: 'idle' })
  | (FrontstageNativePreparationSnapshotBase & {
      status: FrontstageNativePreparationActiveStage;
    })
  | (FrontstageNativePreparationSnapshotBase & {
      status: 'ready';
      prepared: FrontstageNativePreparedRuntime;
      mountIntent: FrontstageNativeInstanceMountIntent | null;
    })
  | (FrontstageNativePreparationSnapshotBase & {
      status: 'failed';
      failedStage: FrontstageNativePreparationActiveStage;
      error: Error;
    })
  | (FrontstageNativePreparationSnapshotBase & { status: 'disposed' });

export interface FrontstageNativePreparationTask {
  blockId: string;
  slotIndex: number;
  identity: string;
  observationContext?: FrontstageRuntimeObservationContext;
  observe?(input: {
    stage: FrontstageNativeRuntimeObservationStage;
    generation: number;
    cacheTier?: FrontstageRuntimeObservationCacheTier;
    timestampMs: number;
    durationMs: number;
  }): void;
  prepare(
    signal: AbortSignal,
    enterStage: (
      stage: FrontstageNativePreparationActiveStage,
      cacheTier?: FrontstageRuntimeObservationCacheTier
    ) => void
  ): Promise<FrontstageNativePreparedRuntime>;
}

interface ScheduledPreparation {
  task: FrontstageNativePreparationTask;
  priority: FrontstageRuntimeDemandPriority;
  generation: number;
  snapshot: FrontstageNativePreparationSnapshot;
  abortController: AbortController | null;
  observedAtMs: number;
}

export const DEFAULT_FRONTSTAGE_NATIVE_PREPARATION_CONCURRENCY = 2;

/** Owns one page's bounded Native React preparation queue; React roots belong to P2. */
export class FrontstageNativePreparationScheduler {
  private readonly scheduled = new Map<string, ScheduledPreparation>();
  private readonly listeners = new Set<() => void>();
  private visible = true;

  constructor(
    readonly maxConcurrent = DEFAULT_FRONTSTAGE_NATIVE_PREPARATION_CONCURRENCY
  ) {
    if (!Number.isSafeInteger(maxConcurrent) || maxConcurrent < 1) {
      throw new Error(
        'Native preparation concurrency must be a positive integer.'
      );
    }
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getSnapshots(): FrontstageNativePreparationSnapshot[] {
    return [...this.scheduled.values()]
      .sort(
        (left, right) =>
          left.priority - right.priority ||
          left.task.slotIndex - right.task.slotIndex
      )
      .map(({ snapshot }) => snapshot);
  }

  reconcile(
    tasks: readonly FrontstageNativePreparationTask[],
    demands: FrontstageRuntimeDemandByBlockId | undefined
  ): void {
    let snapshotsChanged = false;
    const nextBlockIds = new Set(tasks.map(({ blockId }) => blockId));
    for (const [blockId, current] of this.scheduled) {
      if (!nextBlockIds.has(blockId)) {
        current.abortController?.abort();
        current.generation += 1;
        current.snapshot = this.snapshot(current, 'disposed');
        this.scheduled.delete(blockId);
        snapshotsChanged = true;
      }
    }

    for (const task of tasks) {
      const priority =
        demands?.[task.blockId] ?? (task.slotIndex === 0 ? 1 : 3);
      let current = this.scheduled.get(task.blockId);
      if (!current) {
        current = {
          task,
          priority,
          generation: 0,
          snapshot: {
            blockId: task.blockId,
            slotIndex: task.slotIndex,
            priority,
            generation: 0,
            status: 'idle'
          },
          abortController: null,
          observedAtMs: 0
        };
        this.scheduled.set(task.blockId, current);
        snapshotsChanged = true;
      } else if (current.task.identity !== task.identity) {
        current.abortController?.abort();
        current.task = task;
        current.priority = priority;
        current.generation += 1;
        current.abortController = null;
        current.snapshot = this.snapshot(current, 'idle');
        snapshotsChanged = true;
      } else {
        const placementChanged =
          current.priority !== priority ||
          current.task.slotIndex !== task.slotIndex ||
          !sameObservationContext(
            current.task.observationContext,
            task.observationContext
          );
        current.task = task;
        current.priority = priority;
        if (placementChanged && current.snapshot.status === 'ready') {
          current.snapshot = this.readySnapshot(
            current,
            current.snapshot.prepared,
            current.snapshot.mountIntent
          );
          snapshotsChanged = true;
        } else if (placementChanged) {
          current.snapshot = {
            ...current.snapshot,
            ...this.baseSnapshot(current)
          };
          snapshotsChanged = true;
        }
      }

      if (resolveFrontstageRuntimePreparationKind(priority) === 'dormant') {
        if (current.abortController) {
          current.abortController.abort();
          current.abortController = null;
          current.generation += 1;
          snapshotsChanged = true;
        }
        if (
          current.snapshot.status !== 'ready' &&
          current.snapshot.status !== 'idle'
        ) {
          current.snapshot = this.snapshot(current, 'idle');
          snapshotsChanged = true;
        }
      }
    }
    if (snapshotsChanged) this.emit();
    this.pump();
  }

  setPageVisible(visible: boolean): void {
    if (this.visible === visible) return;
    this.visible = visible;
    if (visible) this.pump();
  }

  retry(blockId: string): void {
    const current = this.scheduled.get(blockId);
    if (!current) return;
    current.abortController?.abort();
    current.abortController = null;
    current.generation += 1;
    current.snapshot = this.snapshot(current, 'idle');
    this.emit();
    this.pump();
  }

  dispose(): void {
    for (const current of this.scheduled.values()) {
      current.abortController?.abort();
      current.abortController = null;
      current.generation += 1;
      current.snapshot = this.snapshot(current, 'disposed');
    }
    this.emit();
    this.scheduled.clear();
  }

  private pump(): void {
    if (!this.visible) return;
    const running = [...this.scheduled.values()].filter(
      ({ abortController }) => abortController !== null
    ).length;
    const available = this.maxConcurrent - running;
    if (available <= 0) return;

    const candidates = createFrontstageRuntimeDemandCandidates(
      [...this.scheduled.values()].map((current) => ({
        blockId: current.task.blockId,
        slotIndex: current.task.slotIndex,
        current
      })),
      Object.fromEntries(
        [...this.scheduled.values()].map((current) => [
          current.task.blockId,
          current.priority
        ])
      )
    )
      .filter(({ value: { current } }) => current.snapshot.status === 'idle')
      .slice(0, available);

    for (const {
      value: { current }
    } of candidates)
      this.start(current);
  }

  private start(current: ScheduledPreparation): void {
    const abortController = new AbortController();
    const generation = current.generation;
    current.abortController = abortController;
    current.snapshot = this.snapshot(current, 'source_fetch');
    current.observedAtMs = Date.now();
    this.observe(current, generation, 'source_fetch', 'network');
    this.emit();

    const enterStage = (
      stage: FrontstageNativePreparationActiveStage,
      cacheTier?: FrontstageRuntimeObservationCacheTier
    ) => {
      if (!this.isCurrent(current, generation, abortController)) return;
      current.snapshot = this.snapshot(current, stage);
      this.observe(current, generation, stage, cacheTier);
      this.emit();
    };
    void current.task
      .prepare(abortController.signal, enterStage)
      .then((prepared) => {
        if (!this.isCurrent(current, generation, abortController)) return;
        current.abortController = null;
        current.snapshot = this.readySnapshot(current, prepared);
        this.emit();
        this.pump();
      })
      .catch((error: unknown) => {
        if (!this.isCurrent(current, generation, abortController)) return;
        const failedStage =
          current.snapshot.status === 'source_fetch' ||
          current.snapshot.status === 'artifact_lookup' ||
          current.snapshot.status === 'compile' ||
          current.snapshot.status === 'module_resolve'
            ? current.snapshot.status
            : 'source_fetch';
        current.abortController = null;
        current.snapshot = {
          ...this.baseSnapshot(current),
          status: 'failed',
          failedStage,
          error: toError(error)
        };
        this.emit();
        this.pump();
      });
  }

  private isCurrent(
    current: ScheduledPreparation,
    generation: number,
    abortController: AbortController
  ): boolean {
    return (
      !abortController.signal.aborted &&
      current.generation === generation &&
      current.abortController === abortController &&
      this.scheduled.get(current.task.blockId) === current
    );
  }

  private observe(
    current: ScheduledPreparation,
    generation: number,
    stage: FrontstageNativePreparationActiveStage,
    cacheTier?: FrontstageRuntimeObservationCacheTier
  ): void {
    const timestampMs = Date.now();
    current.task.observe?.({
      stage,
      generation,
      cacheTier,
      timestampMs,
      durationMs: Math.max(0, timestampMs - current.observedAtMs)
    });
    current.observedAtMs = timestampMs;
  }

  private readySnapshot(
    current: ScheduledPreparation,
    prepared: FrontstageNativePreparedRuntime,
    retainedMountIntent: FrontstageNativeInstanceMountIntent | null = null
  ): FrontstageNativePreparationSnapshot {
    const shouldRetainMount =
      retainedMountIntent !== null || current.priority <= 1;
    return {
      ...this.baseSnapshot(current),
      status: 'ready',
      prepared,
      mountIntent: shouldRetainMount
        ? {
            blockId: current.task.blockId,
            slotIndex: current.task.slotIndex,
            identityInput: prepared.identityInput
          }
        : null
    };
  }

  private snapshot(
    current: ScheduledPreparation,
    status: 'idle' | FrontstageNativePreparationActiveStage | 'disposed'
  ): FrontstageNativePreparationSnapshot {
    return { ...this.baseSnapshot(current), status };
  }

  private baseSnapshot(
    current: ScheduledPreparation
  ): FrontstageNativePreparationSnapshotBase {
    return {
      blockId: current.task.blockId,
      slotIndex: current.task.slotIndex,
      priority: current.priority,
      generation: current.generation,
      observationContext: current.task.observationContext
    };
  }

  private emit(): void {
    for (const listener of this.listeners) listener();
  }
}

function sameObservationContext(
  left: FrontstageRuntimeObservationContext | undefined,
  right: FrontstageRuntimeObservationContext | undefined
): boolean {
  return (
    left === right ||
    (left !== undefined &&
      right !== undefined &&
      left.actorId === right.actorId &&
      left.workspaceId === right.workspaceId &&
      left.pageId === right.pageId &&
      left.tabId === right.tabId &&
      left.blockId === right.blockId)
  );
}

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('Native React preparation failed.');
}
