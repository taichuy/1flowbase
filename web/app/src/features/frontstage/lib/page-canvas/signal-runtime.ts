import type { BlockContextOutputs } from '@1flowbase/page-protocol';

import type { FrontstageBlockInstance } from '../page-document';
import { createFrontstageSignalGraph } from '../page-signals/graph';
import {
  clearFrontstagePageSignals,
  commitFrontstageBlockOutputs,
  createFrontstageSignalSnapshot,
  readFrontstageSignal,
  type FrontstageSignalSnapshot
} from '../page-signals/store';

export interface FrontstageSignalRuntimeCommitResult {
  ok: boolean;
  stale: boolean;
  error?: string;
}

export interface FrontstagePageSignalSession {
  snapshot: FrontstageSignalSnapshot;
}

export interface FrontstageBlockSignalSnapshot {
  readonly revision: number;
  readonly inputs: Readonly<Record<string, unknown>>;
}

export type FrontstageBlockSignalListener = () => void;

export function createFrontstagePageSignalSession(): FrontstagePageSignalSession {
  return { snapshot: createFrontstageSignalSnapshot() };
}

export class FrontstageSignalRuntimeCoordinator {
  graph;
  private blocksById;
  private readonly tabId;
  private readonly pageSession;
  private latestInstanceEpochs = new Map<string, string>();
  private blockSnapshots = new Map<string, FrontstageBlockSignalSnapshot>();
  private blockListeners = new Map<
    string,
    Set<FrontstageBlockSignalListener>
  >();
  private nextInstanceEpoch = 0;

  constructor(
    blocks: readonly FrontstageBlockInstance[],
    tabId: string,
    pageSession: FrontstagePageSignalSession = createFrontstagePageSignalSession()
  ) {
    this.blocksById = new Map(blocks.map((block) => [block.id, block]));
    this.tabId = tabId;
    this.pageSession = pageSession;
    this.graph = createFrontstageSignalGraph(blocks);
  }

  updateBlocks(blocks: readonly FrontstageBlockInstance[]): void {
    this.blocksById = new Map(blocks.map((block) => [block.id, block]));
    this.graph = createFrontstageSignalGraph(blocks);
    for (const blockId of this.latestInstanceEpochs.keys()) {
      if (!this.blocksById.has(blockId)) {
        this.latestInstanceEpochs.delete(blockId);
      }
    }
    for (const blockId of this.blockSnapshots.keys()) {
      if (!this.blocksById.has(blockId)) this.blockSnapshots.delete(blockId);
    }
    for (const blockId of this.blockListeners.keys()) {
      if (!this.blocksById.has(blockId)) this.blockListeners.delete(blockId);
    }
  }

  get revision(): number {
    return this.pageSession.snapshot.revision;
  }

  instanceEpochFor(blockId: string): string | null {
    return this.latestInstanceEpochs.get(blockId) ?? null;
  }

  beginInstance(blockId: string, epoch?: string): string {
    const nextEpoch = epoch ?? `${blockId}:${++this.nextInstanceEpoch}`;
    this.latestInstanceEpochs.set(blockId, nextEpoch);
    return nextEpoch;
  }

  endInstance(blockId: string, epoch: string): void {
    if (this.latestInstanceEpochs.get(blockId) === epoch) {
      this.latestInstanceEpochs.delete(blockId);
    }
  }

  outputsFor(blockId: string, epoch: string): BlockContextOutputs {
    return {
      publish: (values) => this.commit(blockId, epoch, values)
    };
  }

  subscribeBlock(
    blockId: string,
    listener: FrontstageBlockSignalListener
  ): () => void {
    const listeners = this.blockListeners.get(blockId) ?? new Set();
    listeners.add(listener);
    this.blockListeners.set(blockId, listeners);
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0 && this.blockListeners.get(blockId) === listeners)
        this.blockListeners.delete(blockId);
    };
  }

  getBlockSnapshot(blockId: string): FrontstageBlockSignalSnapshot {
    const current = this.blockSnapshots.get(blockId);
    const inputs = this.inputsFor(blockId);
    if (current && inputsEqual(current.inputs, inputs)) return current;
    const snapshot = createBlockSnapshot(this.revision, inputs);
    this.blockSnapshots.set(blockId, snapshot);
    return snapshot;
  }

  canRun(blockId: string): boolean {
    const block = this.blocksById.get(blockId);
    if (
      !block ||
      this.graph.diagnostics.some((item) => item.block_id === blockId)
    )
      return false;
    return (block.ports?.inputs ?? []).every(
      (input) => !input.source || this.readSource(input.source) !== undefined
    );
  }

  inputsFor(blockId: string): Record<string, unknown> {
    const inputs: Record<string, unknown> = {};
    const block = this.blocksById.get(blockId);
    for (const input of block?.ports?.inputs ?? []) {
      if (!input.source) continue;
      const value = this.readSource(input.source);
      if (value !== undefined) inputs[input.name] = value;
    }
    return inputs;
  }

  inputSignature(blockId: string): string {
    return JSON.stringify(this.inputsFor(blockId));
  }

  commit(
    blockId: string,
    instanceEpoch: string,
    outputs: Record<string, unknown>
  ): FrontstageSignalRuntimeCommitResult {
    if (this.latestInstanceEpochs.get(blockId) !== instanceEpoch)
      return { ok: false, stale: true };
    const block = this.blocksById.get(blockId);
    if (!block)
      return { ok: false, stale: false, error: 'Signal block does not exist.' };
    const committed = commitFrontstageBlockOutputs({
      block,
      outputs,
      scopes: this.outputScopes(blockId),
      tabId: this.tabId,
      snapshot: this.pageSession.snapshot
    });
    if (!committed.ok)
      return { ok: false, stale: false, error: committed.error };
    this.pageSession.snapshot = committed.snapshot;
    const affectedBlocks = this.graph.order.filter((candidateId) =>
      this.graph.dependencies.get(candidateId)?.has(blockId)
    );
    for (const affectedBlockId of affectedBlocks) {
      this.blockSnapshots.set(
        affectedBlockId,
        createBlockSnapshot(this.revision, this.inputsFor(affectedBlockId))
      );
    }
    for (const affectedBlockId of affectedBlocks) {
      const listeners = this.blockListeners.get(affectedBlockId);
      if (!listeners) continue;
      for (const listener of [...listeners]) {
        if (listeners.has(listener)) listener();
      }
    }
    return { ok: true, stale: false };
  }

  clear(): void {
    this.pageSession.snapshot = clearFrontstagePageSignals();
    this.latestInstanceEpochs.clear();
    this.blockSnapshots.clear();
    this.blockListeners.clear();
    this.nextInstanceEpoch = 0;
  }

  dispose(): void {
    this.clear();
  }

  private readSource(
    source: NonNullable<
      NonNullable<FrontstageBlockInstance['ports']>['inputs'][number]['source']
    >
  ): unknown {
    return readFrontstageSignal(this.pageSession.snapshot, {
      scope: source.scope,
      tab_id: source.tab_id ?? this.tabId,
      block_id: source.block_id,
      output: source.output
    });
  }

  private outputScopes(blockId: string): Array<'tab' | 'page'> {
    const scopes = new Set<'tab' | 'page'>(['tab']);
    for (const block of this.blocksById.values()) {
      for (const input of block.ports?.inputs ?? []) {
        if (input.source?.block_id === blockId) scopes.add(input.source.scope);
      }
    }
    return [...scopes];
  }
}

function createBlockSnapshot(
  revision: number,
  inputs: Record<string, unknown>
): FrontstageBlockSignalSnapshot {
  return Object.freeze({ revision, inputs: Object.freeze(inputs) });
}

function inputsEqual(
  current: Readonly<Record<string, unknown>>,
  next: Readonly<Record<string, unknown>>
): boolean {
  const currentNames = Object.keys(current);
  const nextNames = Object.keys(next);
  return (
    currentNames.length === nextNames.length &&
    currentNames.every(
      (name) => Object.hasOwn(next, name) && Object.is(current[name], next[name])
    )
  );
}
