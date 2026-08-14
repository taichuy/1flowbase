import { useQueryClient } from '@tanstack/react-query';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';
import { useCallback, useEffect, useMemo, useState } from 'react';

import {
  fetchFrontstageBlockNode,
  fetchFrontstageBlockNodeCode,
  frontstageBlockTreeQueryKeys,
  saveFrontstageBlockNodeCode,
  type FrontstageBlockNode,
  type FrontstageBlockNodeCode
} from '../../../api/block-tree';
import { useAuthStore } from '../../../../../state/auth-store';
import type { FrontstageBlockDeletedEvent } from './types';

export interface FrontstageBlockCodeTabState {
  block_id: string;
  detail: FrontstageBlockNode | null;
  base_source: string;
  draft: string;
  source_sha256: string | null;
  executable: FrontstageBlockNodeCode | null;
  loading: boolean;
  saving: boolean;
  error: unknown | null;
}

function pendingTab(blockId: string): FrontstageBlockCodeTabState {
  return {
    block_id: blockId,
    detail: null,
    base_source: '',
    draft: '',
    source_sha256: null,
    executable: null,
    loading: true,
    saving: false,
    error: null
  };
}

export interface FrontstageExecutableSavePayload {
  source_code: string;
  dependency_lock: NativeReactCatalogDependencyLock;
  expected_source_revision: string | null;
}

export async function compileFrontstageExecutableSave(
  tab: FrontstageBlockCodeTabState
): Promise<FrontstageExecutableSavePayload> {
  if (!tab.executable) {
    throw new Error('Frontstage block source state is missing.');
  }
  const dependencyLock = canonicalizeNativeReactCatalogDependencyLock(
    tab.executable.dependency_lock ?? []
  );
  if (!dependencyLock) {
    throw new Error('Frontstage block dependency_lock is invalid.');
  }
  return {
    source_code: tab.draft,
    expected_source_revision: tab.source_sha256,
    dependency_lock: dependencyLock.filter(
      ({ module_source }) => module_source !== 'tailwindcss'
    )
  };
}

function errorStatus(error: unknown): number | null {
  return error !== null &&
    typeof error === 'object' &&
    'status' in error &&
    typeof error.status === 'number'
    ? error.status
    : null;
}

function requireCsrfToken(csrfToken: string | null): string {
  if (!csrfToken) throw new Error('missing csrf token');
  return csrfToken;
}

export function useFrontstageBlockTabs({
  workspaceId,
  pageId,
  initialBlockId,
  open
}: {
  workspaceId: string;
  pageId: string;
  initialBlockId: string;
  open: boolean;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [tabs, setTabs] = useState<FrontstageBlockCodeTabState[]>(() => [
    pendingTab(initialBlockId)
  ]);
  const [activeBlockId, setActiveBlockId] = useState(initialBlockId);

  const updateTab = useCallback(
    (
      blockId: string,
      update: (tab: FrontstageBlockCodeTabState) => FrontstageBlockCodeTabState
    ) => {
      setTabs((current) =>
        current.map((tab) => (tab.block_id === blockId ? update(tab) : tab))
      );
    },
    []
  );

  const loadBlock = useCallback(
    async (blockId: string) => {
      updateTab(blockId, (tab) => ({
        ...tab,
        loading: true,
        error: null
      }));
      try {
        const [detail, code] = await Promise.all([
          fetchFrontstageBlockNode(workspaceId, pageId, blockId),
          fetchFrontstageBlockNodeCode(workspaceId, pageId, blockId)
        ]);
        queryClient.setQueryData(
          frontstageBlockTreeQueryKeys.block(workspaceId, pageId, blockId),
          detail
        );
        queryClient.setQueryData(
          frontstageBlockTreeQueryKeys.code(workspaceId, pageId, blockId),
          code
        );
        updateTab(blockId, (tab) => ({
          ...tab,
          detail,
          base_source: code.source_code,
          draft: code.source_code,
          source_sha256: code.source_sha256,
          executable: code,
          loading: false,
          error: null
        }));
      } catch (error) {
        updateTab(blockId, (tab) => ({
          ...tab,
          loading: false,
          error
        }));
      }
    },
    [pageId, queryClient, updateTab, workspaceId]
  );

  useEffect(() => {
    if (!open) return;
    setTabs([pendingTab(initialBlockId)]);
    setActiveBlockId(initialBlockId);
    void loadBlock(initialBlockId);
  }, [initialBlockId, loadBlock, open]);

  const openBlock = useCallback(
    (blockId: string) => {
      const existing = tabs.some((tab) => tab.block_id === blockId);
      if (!existing) {
        setTabs((current) => [...current, pendingTab(blockId)]);
        void loadBlock(blockId);
      }
      setActiveBlockId(blockId);
    },
    [loadBlock, tabs]
  );

  const closeBlock = useCallback(
    (blockId: string) => {
      if (blockId === initialBlockId) return;
      const closingIndex = tabs.findIndex((tab) => tab.block_id === blockId);
      const next = tabs.filter((tab) => tab.block_id !== blockId);
      setTabs(next);
      if (activeBlockId === blockId) {
        const nextActive =
          next[Math.min(Math.max(closingIndex - 1, 0), next.length - 1)] ??
          next[0];
        if (nextActive) setActiveBlockId(nextActive.block_id);
      }
    },
    [activeBlockId, initialBlockId, tabs]
  );

  const activeTab =
    tabs.find((tab) => tab.block_id === activeBlockId) ?? tabs[0];

  const setDraft = useCallback(
    (blockId: string, draft: string) => {
      updateTab(blockId, (tab) => ({ ...tab, draft }));
    },
    [updateTab]
  );

  const setActiveDraft = useCallback(
    (draft: string) => {
      if (!activeTab) return;
      setDraft(activeTab.block_id, draft);
    },
    [activeTab, setDraft]
  );

  const resetActive = useCallback(() => {
    if (!activeTab) return;
    updateTab(activeTab.block_id, (tab) => ({
      ...tab,
      draft: tab.base_source,
      error: null
    }));
  }, [activeTab, updateTab]);

  const saveActiveDraft = useCallback(async () => {
    if (!activeTab) return;
    const blockId = activeTab.block_id;
    updateTab(blockId, (tab) => ({ ...tab, saving: true, error: null }));
    try {
      const payload = await compileFrontstageExecutableSave(activeTab);
      const code = await saveFrontstageBlockNodeCode(
        workspaceId,
        pageId,
        blockId,
        payload,
        requireCsrfToken(csrfToken)
      );
      queryClient.setQueryData(
        frontstageBlockTreeQueryKeys.code(workspaceId, pageId, blockId),
        code
      );
      updateTab(blockId, (tab) => ({
        ...tab,
        base_source: code.source_code,
        draft: code.source_code,
        source_sha256: code.source_sha256,
        executable: code,
        saving: false,
        error: null
      }));
    } catch (error) {
      updateTab(blockId, (tab) => ({
        ...tab,
        saving: false,
        error
      }));
      throw error;
    }
  }, [activeTab, csrfToken, pageId, queryClient, updateTab, workspaceId]);

  const handleDeletedBlock = useCallback(
    async (event: FrontstageBlockDeletedEvent) => {
      if (event.block_id === initialBlockId)
        return 'initial_root_deleted' as const;

      const openBlockIds = tabs.map((tab) => tab.block_id);
      setTabs((current) =>
        current.filter((tab) => tab.block_id !== event.block_id)
      );
      if (activeBlockId === event.block_id) {
        setActiveBlockId(initialBlockId);
      }
      if (!event.subtree) return 'converged' as const;

      await Promise.all(
        openBlockIds
          .filter((blockId) => blockId !== event.block_id)
          .map(async (blockId) => {
            try {
              const detail = await fetchFrontstageBlockNode(
                workspaceId,
                pageId,
                blockId
              );
              updateTab(blockId, (tab) => ({ ...tab, detail, error: null }));
            } catch (error) {
              if (errorStatus(error) === 404) {
                setTabs((current) =>
                  current.filter((tab) => tab.block_id !== blockId)
                );
                setActiveBlockId((current) =>
                  current === blockId ? initialBlockId : current
                );
                return;
              }
              updateTab(blockId, (tab) => ({ ...tab, error }));
            }
          })
      );
      return 'converged' as const;
    },
    [activeBlockId, initialBlockId, pageId, tabs, updateTab, workspaceId]
  );

  const anyDirty = useMemo(
    () => tabs.some((tab) => tab.draft !== tab.base_source),
    [tabs]
  );

  return {
    tabs,
    activeBlockId,
    activeTab,
    anyDirty,
    openBlock,
    activateBlock: setActiveBlockId,
    closeBlock,
    setDraft,
    setActiveDraft,
    resetActive,
    saveActiveDraft,
    handleDeletedBlock
  };
}
