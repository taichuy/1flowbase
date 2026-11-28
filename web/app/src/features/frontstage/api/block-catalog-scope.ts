export const frontstageBlockCatalogQueryStaleTime = 30_000;

export function createFrontstageBlockCatalogPermissionFingerprint(me: {
  policy_fingerprint: string;
  effective_display_role: string;
}): string {
  return JSON.stringify([me.effective_display_role, me.policy_fingerprint]);
}
