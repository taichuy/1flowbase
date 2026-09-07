import type {
  FrontstageNativePreparationSnapshot,
  FrontstageNativePreparationSource
} from '../../../lib/page-canvas/native-runtime-preparation';

/** Static injected runtime for canvas fixtures; live tests use the real scheduler. */
export function createNativePreparationSource(
  snapshots: readonly FrontstageNativePreparationSnapshot[] = []
): FrontstageNativePreparationSource {
  const byBlock = new Map(
    snapshots.map((snapshot) => [snapshot.blockId, snapshot])
  );
  return {
    getBlockSnapshot: (blockId) => byBlock.get(blockId) ?? null,
    subscribeBlock: () => () => {}
  };
}
