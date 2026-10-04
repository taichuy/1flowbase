import { createContext, useContext, useLayoutEffect } from 'react';

export const FrontstageRetentionProtectionContext = createContext<
  ((protectedFromEviction: boolean) => void) | null
>(null);

/** Known editor drafts and in-flight writes outlive ordinary cache pressure. */
export function useFrontstageRetentionProtection(
  protectedFromEviction: boolean
) {
  const update = useContext(FrontstageRetentionProtectionContext);
  useLayoutEffect(() => {
    update?.(protectedFromEviction);
    return () => update?.(false);
  }, [protectedFromEviction, update]);
}
