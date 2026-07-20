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
  readonly graph;
  private readonly blocksById;
  private readonly tabId;
  private readonly pageSession;
  private latestRunIds = new Map<string, string>();

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

  get revision(): number {
    return this.pageSession.snapshot.revision;
  }

  beginRun(blockId: string, runId: string): void {
    this.latestRunIds.set(blockId, runId);
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
    runId: string,
    outputs: Record<string, unknown>
  ): FrontstageSignalRuntimeCommitResult {
    if (this.latestRunIds.get(blockId) !== runId)
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
    this.latestRunIds.clear();
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
