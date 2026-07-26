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

export function createFrontstagePageSignalSession(): FrontstagePageSignalSession {
  return { snapshot: createFrontstageSignalSnapshot() };
}

export class FrontstageSignalRuntimeCoordinator {
  graph;
  private blocksById;
  private readonly tabId;
  private readonly pageSession;
  private latestInstanceEpochs = new Map<string, string>();
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

  outputsFor(
    blockId: string,
    epoch: string,
    onPublish?: (revision: number) => void
  ): BlockContextOutputs {
    return {
      publish: (values) => {
        const result = this.commit(blockId, epoch, values);
        if (result.ok) onPublish?.(this.revision);
        return result;
      }
    };
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
    const scopes = this.outputScopes(blockId);
    let next = this.pageSession.snapshot;
    for (const scope of scopes) {
      const committed = commitFrontstageBlockOutputs({
        block,
        outputs,
        scope,
        tabId: this.tabId,
        snapshot: next
      });
      if (!committed.ok)
        return { ok: false, stale: false, error: committed.error };
      next = committed.snapshot;
    }
    this.pageSession.snapshot = next;
    return { ok: true, stale: false };
  }

  clear(): void {
    this.pageSession.snapshot = clearFrontstagePageSignals();
    this.latestInstanceEpochs.clear();
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
