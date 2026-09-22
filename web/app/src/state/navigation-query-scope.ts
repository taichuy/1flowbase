import { useAuthStore } from './auth-store';

export const navigationQueryStaleTime = 30_000;

// Navigation is shared by the shell and route containers, but never across
// actors, workspaces, active roles, or effective permission snapshots.
export function selectNavigationQueryScope(
  state: ReturnType<typeof useAuthStore.getState>
): string {
  return JSON.stringify([
    state.actor?.id ?? null,
    state.actor?.current_workspace_id ?? null,
    state.actor?.effective_display_role ?? null,
    [...(state.me?.permissions ?? [])].sort()
  ]);
}
