import { createNativeReactArtifactStore } from './native-react-artifact-store';
import { FrontstageNativeReactArtifactCache } from './native-react-artifact-cache';
import { FrontstageNativeBlockSourceCache } from './native-block-source-cache';

const FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE =
  '1flowbase-frontstage-native-react-artifacts';
const FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_DATABASE =
  '1flowbase-frontstage-native-block-sources';

export const frontstageNativeReactArtifactCache =
  new FrontstageNativeReactArtifactCache({
    store: createNativeReactArtifactStore({
      databaseName: FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE
    })
  });

export const frontstageNativeBlockSourceCache =
  new FrontstageNativeBlockSourceCache({
    databaseName: FRONTSTAGE_NATIVE_BLOCK_SOURCE_CACHE_DATABASE
  });

export * from './indexeddb-store';
export * from './native-react-artifact-cache';
export * from './native-block-source-cache';
