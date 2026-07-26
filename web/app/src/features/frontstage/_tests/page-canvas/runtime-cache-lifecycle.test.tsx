import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { useFrontstageRuntimeCacheLifecycle } from '../../hooks/use-frontstage-runtime-cache-lifecycle';
import {
  clearFrontstageRuntimeSessionCache,
  frontstageRuntimeResultCache,
  readFrontstageRuntimeSessionCacheSize
} from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import {
  FrontstageRuntimeObservationBuffer,
  readFrontstageRuntimeObservations,
  recordFrontstageRuntimeObservation,
  resetFrontstageRuntimeObservations
} from '../../lib/page-canvas/runtime-observation';

function authenticate(actorId: string) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: `csrf-${actorId}`,
    actor: {
      id: actorId,
      account: actorId,
      effective_display_role: 'developer',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
}

function seedActorScopedRuntimeState(
  queryClient: QueryClient,
  actorId: string
) {
  frontstageRuntimeResultCache.set(`runtime:${actorId}`, {
    view: { primitive: 'Text', props: { children: actorId } },
    outputs: { actorId },
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedDataPermissions: [],
      allowedEvents: []
    }
  });
  queryClient.setQueryData(
    ['frontstage', actorId, 'workspace-1', 'pages', 'page-1'],
    { actorId }
  );
  recordFrontstageRuntimeObservation({
    stage: 'present',
    cacheTier: 'l1',
    actorId,
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    tabId: 'tab-1',
    blockId: 'block-1'
  });
}

describe('frontstage runtime cache lifecycle', () => {
  beforeEach(() => {
    resetAuthStore();
    clearFrontstageRuntimeSessionCache();
    resetFrontstageRuntimeObservations();
  });

  test('clears L1, frontstage queries, and observations for anonymous and actor transitions', async () => {
    const queryClient = new QueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
    authenticate('actor-a');
    renderHook(() => useFrontstageRuntimeCacheLifecycle(), { wrapper });

    seedActorScopedRuntimeState(queryClient, 'actor-a');
    expect(readFrontstageRuntimeSessionCacheSize()).toBe(1);
    expect(
      queryClient.getQueryCache().findAll({ queryKey: ['frontstage'] })
    ).toHaveLength(1);
    expect(readFrontstageRuntimeObservations()).toHaveLength(1);

    act(() => useAuthStore.getState().setAnonymous());
    await waitFor(() => {
      expect(readFrontstageRuntimeSessionCacheSize()).toBe(0);
      expect(
        queryClient.getQueryCache().findAll({ queryKey: ['frontstage'] })
      ).toHaveLength(0);
      expect(readFrontstageRuntimeObservations()).toEqual([]);
    });

    seedActorScopedRuntimeState(queryClient, 'anonymous');
    act(() => authenticate('actor-b'));
    await waitFor(() => {
      expect(readFrontstageRuntimeSessionCacheSize()).toBe(0);
      expect(
        queryClient.getQueryCache().findAll({ queryKey: ['frontstage'] })
      ).toHaveLength(0);
      expect(readFrontstageRuntimeObservations()).toEqual([]);
    });
  });

  test('bounds observation entries while preserving stage counts', () => {
    const observations = new FrontstageRuntimeObservationBuffer(2);
    const base = {
      cacheTier: 'runtime' as const,
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      blockId: 'block-1'
    };
    observations.record({ ...base, stage: 'worker_boot' });
    observations.record({ ...base, stage: 'present' });
    observations.record({ ...base, stage: 'present' });

    expect(observations.read()).toEqual([
      expect.objectContaining({ sequence: 2, stage: 'present', count: 1 }),
      expect.objectContaining({ sequence: 3, stage: 'present', count: 2 })
    ]);
  });

  test('AC-022 starts non-blocking persistent prune and purges the previous actor on logout/switch', async () => {
    const queryClient = new QueryClient();
    const artifactCache = {
      deleteActor: vi.fn(async () => ({
        status: 'completed' as const,
        deleted: 0
      })),
      pruneWorkspace: vi.fn(async () => ({
        status: 'completed' as const,
        deleted: 0
      }))
    };
    const nativeReactArtifactCache = {
      deleteActor: vi.fn(async () => ({
        status: 'completed' as const,
        deleted: 0
      })),
      pruneWorkspace: vi.fn(async () => ({
        status: 'completed' as const,
        deleted: 0
      }))
    };
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
    authenticate('actor-a');
    renderHook(
      () =>
        useFrontstageRuntimeCacheLifecycle({
          artifactCache,
          nativeReactArtifactCache,
          runtimeFingerprint: 'runtime-a',
          nativeReactRuntimeFingerprint: 'native-runtime-a'
        }),
      { wrapper }
    );
    expect(artifactCache.pruneWorkspace).toHaveBeenCalledWith({
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      runtimeFingerprint: 'runtime-a'
    });
    expect(nativeReactArtifactCache.pruneWorkspace).toHaveBeenCalledWith({
      actorId: 'actor-a',
      workspaceId: 'workspace-1',
      runtimeFingerprint: 'native-runtime-a'
    });

    act(() => authenticate('actor-b'));
    await waitFor(() => {
      expect(artifactCache.deleteActor).toHaveBeenCalledWith('actor-a');
      expect(nativeReactArtifactCache.deleteActor).toHaveBeenCalledWith(
        'actor-a'
      );
      expect(artifactCache.pruneWorkspace).toHaveBeenLastCalledWith({
        actorId: 'actor-b',
        workspaceId: 'workspace-1',
        runtimeFingerprint: 'runtime-a'
      });
      expect(nativeReactArtifactCache.pruneWorkspace).toHaveBeenLastCalledWith({
        actorId: 'actor-b',
        workspaceId: 'workspace-1',
        runtimeFingerprint: 'native-runtime-a'
      });
    });
    act(() => useAuthStore.getState().setAnonymous());
    await waitFor(() =>
      expect(artifactCache.deleteActor).toHaveBeenLastCalledWith('actor-b')
    );
    expect(nativeReactArtifactCache.deleteActor).toHaveBeenLastCalledWith(
      'actor-b'
    );
  });
});
