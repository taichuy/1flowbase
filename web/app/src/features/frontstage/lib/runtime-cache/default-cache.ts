import { createIndexedDbRecordStore } from './indexeddb-store';
import {
  FrontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCacheRecord
} from './native-react-artifact-cache';

export const FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE =
  '1flowbase-frontstage-native-react-artifacts';

export const frontstageNativeReactArtifactCache =
  new FrontstageNativeReactArtifactCache({
    store: createIndexedDbRecordStore<FrontstageNativeReactArtifactCacheRecord>(
      {
        databaseName: FRONTSTAGE_NATIVE_REACT_ARTIFACT_CACHE_DATABASE
      }
    )
  });

export * from './indexeddb-store';
export * from './native-react-artifact-cache';
