type PublicAuthModule = typeof import('../components/PublicAuthBlock');
let runtimeFlight: Promise<PublicAuthModule> | undefined;
let intentGeneration = 0;

export function loadPublicAuthBlock(): Promise<PublicAuthModule> {
  runtimeFlight ??= import('../components/PublicAuthBlock').catch((error) => {
    runtimeFlight = undefined;
    throw error;
  });
  return runtimeFlight;
}

export function canPrefetchPublicAuth(): boolean {
  const connection = (
    navigator as Navigator & {
      connection?: { saveData?: boolean; effectiveType?: string };
    }
  ).connection;
  return (
    !connection?.saveData &&
    !/^(slow-)?2g$/.test(connection?.effectiveType ?? '')
  );
}

export function preloadPublicAuthRuntime(): void {
  if (!canPrefetchPublicAuth()) return;
  // A failed speculative import is retried by the real lazy-loading boundary.
  void loadPublicAuthBlock().catch(() => undefined);
}

export function prefetchPublicAuthEntry(source: string): void {
  const generation = ++intentGeneration;
  if (!canPrefetchPublicAuth()) return;
  void loadPublicAuthBlock()
    .then(() => {
      if (generation !== intentGeneration) return;
      return import('./public-auth-compilation').then(
        ({ prefetchPublicAuthSource }) => {
          if (generation === intentGeneration)
            return prefetchPublicAuthSource(source);
        }
      );
    })
    .catch(() => undefined);
}
