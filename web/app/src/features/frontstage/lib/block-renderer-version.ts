export const FRONTSTAGE_BLOCK_RENDERER_VERSION_V1 = 'v1' as const;

export type FrontstageBlockRendererVersion =
  typeof FRONTSTAGE_BLOCK_RENDERER_VERSION_V1;

export function isSupportedFrontstageBlockRendererVersion(
  value: string
): value is FrontstageBlockRendererVersion {
  return value === FRONTSTAGE_BLOCK_RENDERER_VERSION_V1;
}
