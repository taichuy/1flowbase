export type FrontstageRuntimeDemandPriority = 0 | 1 | 2 | 3;

export type FrontstageRuntimeDemandByBlockId = Readonly<
  Record<string, FrontstageRuntimeDemandPriority>
>;

export type FrontstageRuntimePreparationKind =
  | 'prepare_and_mount_intent'
  | 'preload'
  | 'dormant';

export interface FrontstageRuntimeDemandCandidate<T> {
  blockId: string;
  slotIndex: number;
  priority: FrontstageRuntimeDemandPriority;
  preparationKind: Exclude<FrontstageRuntimePreparationKind, 'dormant'>;
  value: T;
}

export interface FrontstageViewportDemandInput {
  previousPriority: FrontstageRuntimeDemandPriority;
  visible: boolean;
  withinEnterMargin: boolean;
  withinExitMargin: boolean;
}

/** Schmitt-trigger demand: enter near eagerly, leave only past a wider margin. */
export function resolveFrontstageViewportDemandPriority({
  previousPriority,
  visible,
  withinEnterMargin,
  withinExitMargin
}: FrontstageViewportDemandInput): 1 | 2 | 3 {
  if (visible) return 1;
  if (previousPriority <= 2) return withinExitMargin ? 2 : 3;
  return withinEnterMargin ? 2 : 3;
}

export function resolveFrontstageRuntimeDemand(
  demands: FrontstageRuntimeDemandByBlockId | undefined,
  blockId: string,
  slotIndex: number
): FrontstageRuntimeDemandPriority {
  return demands?.[blockId] ?? (slotIndex === 0 ? 1 : 3);
}

export function resolveFrontstageRuntimePreparationKind(
  priority: FrontstageRuntimeDemandPriority
): FrontstageRuntimePreparationKind {
  if (priority <= 1) return 'prepare_and_mount_intent';
  if (priority === 2) return 'preload';
  return 'dormant';
}

/** Stable priority/slot ordering is the scheduling truth for page preparation. */
export function createFrontstageRuntimeDemandCandidates<
  T extends {
    blockId: string;
    slotIndex: number;
  }
>(
  values: readonly T[],
  demands: FrontstageRuntimeDemandByBlockId | undefined
): FrontstageRuntimeDemandCandidate<T>[] {
  return values
    .map((value, inputIndex) => {
      const priority = resolveFrontstageRuntimeDemand(
        demands,
        value.blockId,
        value.slotIndex
      );
      return { value, inputIndex, priority };
    })
    .filter(({ priority }) => priority <= 2)
    .sort(
      (left, right) =>
        left.priority - right.priority ||
        left.value.slotIndex - right.value.slotIndex ||
        left.inputIndex - right.inputIndex
    )
    .map(({ value, priority }) => ({
      blockId: value.blockId,
      slotIndex: value.slotIndex,
      priority,
      preparationKind: priority <= 1 ? 'prepare_and_mount_intent' : 'preload',
      value
    }));
}
