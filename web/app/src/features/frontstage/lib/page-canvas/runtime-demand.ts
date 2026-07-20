export type FrontstageRuntimeDemandPriority = 0 | 1 | 2 | 3;

export type FrontstageRuntimeDemandByBlockId = Readonly<
  Record<string, FrontstageRuntimeDemandPriority>
>;

export function resolveFrontstageRuntimeDemand(
  demands: FrontstageRuntimeDemandByBlockId | undefined,
  blockId: string,
  slotIndex: number
): FrontstageRuntimeDemandPriority {
  return demands?.[blockId] ?? (slotIndex === 0 ? 1 : 3);
}
