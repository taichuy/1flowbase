export const frontstageBlockCatalogQueryStaleTime = 30_000;

export function createFrontstageBlockCatalogPermissionFingerprint(me: {
  effective_display_role: string;
  permissions: readonly string[];
}): string {
  return `role:${me.effective_display_role}|permissions:${[...me.permissions]
    .sort()
    .join(',')}`;
}
