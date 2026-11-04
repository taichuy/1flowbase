import { useCallback } from 'react';

import { useAuthStore } from './auth-store';

export function hasConsoleOperationAccess(
  effectiveDisplayRole: string | undefined,
  permissions: readonly string[] | undefined,
  operationId: string
): boolean {
  return effectiveDisplayRole === 'root' || Boolean(permissions?.includes(operationId));
}

/** Consume the backend's effective operation projection; feature access only opens a surface. */
export function useConsoleOperationAccess() {
  const role = useAuthStore((state) => state.actor?.effective_display_role);
  const permissions = useAuthStore((state) => state.me?.permissions);
  return useCallback(
    (operationId: string) => hasConsoleOperationAccess(role, permissions, operationId),
    [role, permissions]
  );
}
