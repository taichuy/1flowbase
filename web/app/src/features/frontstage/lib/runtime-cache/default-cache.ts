import { createNativeReactArtifactStore } from './native-react-artifact-store';
import { FrontstageNativeReactArtifactCache } from './native-react-artifact-cache';

export const FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE =
  '1flowbase-frontstage-native-react-artifacts';

export const frontstageNativeReactArtifactCache =
  new FrontstageNativeReactArtifactCache({
    store: createNativeReactArtifactStore({
      databaseName: FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE
    })
  });

export * from './indexeddb-store';
export * from './native-react-artifact-cache';
