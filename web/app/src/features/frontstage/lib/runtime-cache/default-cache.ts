import { FrontstageCompiledArtifactCache } from './artifact-cache';
import { createIndexedDbArtifactCacheStore } from './indexeddb-store';

export const frontstageCompiledArtifactCache =
  new FrontstageCompiledArtifactCache({
    store: createIndexedDbArtifactCacheStore()
  });

export * from './artifact-cache';
export * from './indexeddb-store';
