import { type PropsWithChildren, useEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { startAuthSessionDiscovery } from '../api/auth-session-discovery';

export function AuthBootstrap({ children }: PropsWithChildren) {
  const setAuthenticated = useAuthStore((state) => state.setAuthenticated);
  const setAnonymous = useAuthStore((state) => state.setAnonymous);

  useEffect(() => {
    let cancelled = false;

    const bootstrap = async () => {
      const discovery = await startAuthSessionDiscovery();
      if (cancelled) return;
      if (discovery.status === 'authenticated') {
        setAuthenticated(discovery.snapshot);
      } else {
        setAnonymous();
      }
    };

    void bootstrap();

    return () => {
      cancelled = true;
    };
  }, [setAuthenticated, setAnonymous]);

  return <>{children}</>;
}
