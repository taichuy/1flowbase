import {
  nativeReactCatalogDependencyLockIdentity,
  sha256Text,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';
import { useEffect, useMemo, useState } from 'react';

import { getNativeReactRuntimeFingerprint } from '../../../shared/code-block/native-react-compiler-browser';
import { prepareNativeReactSource } from '../../../shared/code-block/native-react-source-preparation';
import type { FrontstageBlockRuntimeAssembly } from '../api/block-tree';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import type { FrontstageNativePreparationSnapshot } from '../lib/page-canvas/native-runtime-preparation';
import {
  describeExternalNpmImportFailure,
  type ExternalNpmPackState
} from '../api/external-npm';

export function useFrontstageRuntimeAssembly({
  assembly,
  dependencyLocksByBlockId,
  externalNpm
}: {
  assembly: FrontstageBlockRuntimeAssembly | undefined;
  dependencyLocksByBlockId: Readonly<
    Record<string, NativeReactCatalogDependencyLock>
  >;
  externalNpm: ExternalNpmPackState;
}): FrontstageNativePreparationSnapshot[] {
  const key = useMemo(
    () =>
      assembly?.layers
        .map((layer) => `${layer.block_id}:${layer.source_sha256}`)
        .join('/') ?? '',
    [assembly]
  );
  const [state, setState] = useState<{
    key: string;
    snapshots: FrontstageNativePreparationSnapshot[];
  }>({ key: '', snapshots: [] });

  useEffect(() => {
    if (!assembly || !key) {
      setState({ key: '', snapshots: [] });
      return;
    }
    let active = true;
    setState({
      key,
      snapshots: assembly.layers.map((layer, slotIndex) => ({
        blockId: layer.block_id,
        slotIndex,
        priority: 0,
        generation: 1,
        status: 'compile'
      }))
    });
    void Promise.all(
      assembly.layers.map(async (layer, slotIndex) => {
        if (sha256Text(layer.code) !== layer.source_sha256.toLowerCase()) {
          throw new Error(
            `Block code digest does not match source_sha256 for ${layer.block_id}.`
          );
        }
        const dependencyLock = dependencyLocksByBlockId[layer.block_id] ?? [];
        const runtimeFingerprint =
          getNativeReactRuntimeFingerprint(dependencyLock);
        const dependencyLockIdentity =
          nativeReactCatalogDependencyLockIdentity(dependencyLock);
        const prepared = await prepareNativeReactSource({
          frozenSource: layer.code,
          requestId: `runtime-assembly:${layer.block_id}:${layer.source_sha256}`,
          dependencyLock,
          registryFactory: createFrontstageNativeReactModuleRegistry
        });
        if (!prepared.ok) {
          throw new Error(
            describeExternalNpmImportFailure(
              prepared.diagnostics[0]?.message ??
                `Block runtime preparation failed for ${layer.block_id}.`,
              externalNpm
            )
          );
        }
        return {
          blockId: layer.block_id,
          slotIndex,
          priority: 0,
          generation: 1,
          status: 'ready' as const,
          prepared: {
            artifact: prepared.artifact,
            component: prepared.component,
            artifactCacheTier: 'miss' as const,
            moduleAssets: prepared.moduleAssets,
            identityInput: {
              sourceSha256: layer.source_sha256.toLowerCase(),
              runtimeFingerprint,
              dependencyLockIdentity
            }
          },
          mountIntent: {
            blockId: layer.block_id,
            slotIndex,
            identityInput: {
              sourceSha256: layer.source_sha256.toLowerCase(),
              runtimeFingerprint,
              dependencyLockIdentity
            }
          }
        } satisfies FrontstageNativePreparationSnapshot;
      })
    ).then(
      (snapshots) => {
        if (active) setState({ key, snapshots });
      },
      (error: unknown) => {
        if (!active) return;
        const failed =
          error instanceof Error ? error : new Error(String(error));
        setState({
          key,
          snapshots: assembly.layers.map((layer, slotIndex) => ({
            blockId: layer.block_id,
            slotIndex,
            priority: 0,
            generation: 1,
            status: 'failed',
            failedStage: 'compile',
            error: failed
          }))
        });
      }
    );
    return () => {
      active = false;
    };
  }, [assembly, dependencyLocksByBlockId, externalNpm, key]);

  return state.key === key ? state.snapshots : [];
}
