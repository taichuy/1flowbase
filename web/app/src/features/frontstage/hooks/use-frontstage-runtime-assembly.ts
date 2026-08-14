import { useMemo } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import type { FrontstageBlockRuntimeAssembly } from '../api/block-tree';
import type { ExternalNpmPackState } from '../api/external-npm';
import type { FrontstageRuntimeDemandByBlockId } from '../lib/page-canvas/runtime-demand';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import type { FrontstageNativePreparationSnapshot } from '../lib/page-canvas/native-runtime-preparation';
import { useFrontstagePageCanvasNativePreparations } from './use-frontstage-page-canvas-native-preparations';

export function useFrontstageRuntimeAssembly({
  workspaceId,
  pageId,
  assembly,
  externalNpm
}: {
  workspaceId: string;
  pageId: string | null;
  assembly: FrontstageBlockRuntimeAssembly | undefined;
  externalNpm: ExternalNpmPackState;
}): FrontstageNativePreparationSnapshot[] {
  const actor = useAuthStore((state) => state.actor);
  const readPlan = useMemo<FrontstagePageCanvasBlockCodeReadPlan | null>(() => {
    if (!assembly || !pageId) return null;
    return {
      workspaceId,
      pageId,
      requests: assembly.layers.map((layer, slotIndex) => ({
        requestId: `runtime-assembly:${workspaceId}:${pageId}:${layer.block_id}:${layer.source_revision ?? 'current'}`,
        workspaceId,
        pageId,
        blockId: layer.block_id,
        sourceBlockId: null,
        codeRef: layer.code_ref,
        sourceCodeRef: null,
        runtimeEntry: 'default',
        runtimeKind: 'native_react',
        order: slotIndex,
        sourceIndex: slotIndex,
        slotIndex,
        installationId: null,
        providerCode: null,
        pluginId: null,
        pluginVersion: null,
        contributionCode: 'runtime-assembly'
      }))
    };
  }, [assembly, pageId, workspaceId]);
  const demands = useMemo<FrontstageRuntimeDemandByBlockId | undefined>(
    () =>
      assembly
        ? Object.fromEntries(
            assembly.layers.map((layer) => [layer.block_id, 0])
          )
        : undefined,
    [assembly]
  );
  return useFrontstagePageCanvasNativePreparations({
    actorId: actor?.id,
    actorWorkspaceId: actor?.current_workspace_id,
    readPlan,
    externalNpm,
    demandsByBlockId: demands
  }).preparations;
}
