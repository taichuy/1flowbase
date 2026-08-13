import { nativeReactCatalogDependencyLockIdentity } from '@1flowbase/page-runtime';
import { useEffect, useMemo, useState } from 'react';

import { getNativeReactRuntimeFingerprint } from '../../../shared/code-block/native-react-compiler-browser';
import { prepareNativeReactSource } from '../../../shared/code-block/native-react-source-preparation';
import { readLockedNativeReactExecutableStyle } from '../../../shared/code-block/native-react-executable-style';
import type { FrontstageBlockRuntimeAssembly } from '../api/block-tree';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import type { FrontstageNativePreparationSnapshot } from '../lib/page-canvas/native-runtime-preparation';
import {
  describeExternalNpmImportFailure,
  type ExternalNpmPackState
} from '../api/external-npm';

export function useFrontstageRuntimeAssembly({
  assembly,
  externalNpm
}: {
  assembly: FrontstageBlockRuntimeAssembly | undefined;
  externalNpm: ExternalNpmPackState;
}): FrontstageNativePreparationSnapshot[] {
  const key = useMemo(
    () =>
      assembly?.layers
        .map((layer) => `${layer.block_id}:${layer.generated_css_sha256}`)
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
        const executable = readLockedNativeReactExecutableStyle(layer);
        const dependencyLock = executable.dependency_lock;
        const runtimeFingerprint = getNativeReactRuntimeFingerprint(
          dependencyLock,
          executable.executable_style_identity
        );
        const dependencyLockIdentity =
          nativeReactCatalogDependencyLockIdentity(dependencyLock);
        const prepared = await prepareNativeReactSource({
          frozenSource: executable.source_code,
          requestId: `runtime-assembly:${layer.block_id}:${executable.source_sha256}`,
          dependencyLock,
          runtimeFingerprint,
          executableStyle: {
            generated_css: executable.generated_css,
            generated_css_sha256: executable.generated_css_sha256,
            tailwind_toolchain_lock: executable.tailwind_toolchain_lock,
            compiler_identity: executable.compiler_identity
          },
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
            generatedCssSha256: executable.generated_css_sha256,
            identityInput: {
              sourceSha256: executable.source_sha256,
              runtimeFingerprint,
              dependencyLockIdentity,
              executableStyleIdentity: executable.executable_style_identity
            }
          },
          mountIntent: {
            blockId: layer.block_id,
            slotIndex,
            identityInput: {
              sourceSha256: executable.source_sha256,
              runtimeFingerprint,
              dependencyLockIdentity,
              executableStyleIdentity: executable.executable_style_identity
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
  }, [assembly, externalNpm, key]);

  return state.key === key ? state.snapshots : [];
}
