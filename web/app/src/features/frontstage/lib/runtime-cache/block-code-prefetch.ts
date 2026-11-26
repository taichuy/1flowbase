import { warmDefaultNativeReactCompilerWorker } from '../../../../shared/code-block/native-react-compiler-browser';
import {
  resolveRuntimeConcurrencyProfile,
  type RuntimeConcurrencyProfile
} from '../../../../shared/utils/runtime-concurrency-profile';
import {
  prefetchStartupBlockCodeEntries,
  readStartupPrefetchedBlockNodeCode,
  type StartupPrefetchedBlockCodeEntry
} from '../../api/startup-block-code-cache';
import { prewarmNativeModuleSources } from '../native-modules/registry';

export function prewarmCompilerWorkerModule(): void {
  warmDefaultNativeReactCompilerWorker();
}

/**
 * Warms the browser TSX compiler Worker pool and prewarms native host module
 * dependencies strictly within the initial viewport slot budget (`1` in DEV,
 * `4` first-screen slots in PROD) so offscreen dormant blocks never trigger
 * heavy dependency parsing/evaluation at page open.
 */
export function prewarmStartupBlockRuntime(
  entries: ReadonlyArray<StartupPrefetchedBlockCodeEntry>,
  concurrencyProfile?: RuntimeConcurrencyProfile
): void {
  if (entries.length === 0) return;
  // Do NOT unconditionally spawn or fetch the 90 KB compiler Worker here:
  // warm IndexedDB `artifactCache` hits mount in 0 compilations; the Worker
  // loads lazily only when a visible/near-viewport block misses `artifactCache`.
  const profile = concurrencyProfile ?? resolveRuntimeConcurrencyProfile();
  const targetEntries = entries.slice(
    0,
    Math.max(0, profile.startupPrewarmSlotLimit)
  );
  for (const entry of targetEntries) {
    void entry.promise
      .then((code) => {
        if (code?.source_code) {
          prewarmNativeModuleSources(code.source_code);
        }
      })
      .catch(() => undefined);
  }
}

export function prefetchStartupBlockNodeCodes({
  actorId,
  workspaceId,
  pageId,
  blockIds
}: {
  actorId: string;
  workspaceId: string;
  pageId: string;
  blockIds: readonly string[];
}): void {
  const entries = prefetchStartupBlockCodeEntries({
    actorId,
    workspaceId,
    pageId,
    blockIds
  });
  prewarmStartupBlockRuntime(entries);
}

export const consumeStartupPrefetchedBlockNodeCode =
  readStartupPrefetchedBlockNodeCode;
