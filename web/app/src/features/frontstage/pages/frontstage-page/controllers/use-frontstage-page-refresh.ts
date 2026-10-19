import { useCallback, useEffect, useRef, useState } from 'react';

import type { FrontstagePageContent } from '../../../api/page-content';

import { useFrontstagePageCanvasNativePreparations } from '../../../hooks/use-frontstage-page-canvas-native-preparations';

import { useFrontstageRuntimeAssembly } from '../../../hooks/use-frontstage-runtime-assembly';

import { type FrontstageBlockInstance } from '../../../lib/page-document';

import { toDisplayErrorMessage } from '../page-action-helpers';

import type { FrontStagePageProps } from '../page-props';

import type { Dispatch, SetStateAction } from 'react';

/** Owns refresh generation fencing and dependent native preparation refresh. */
export function useFrontstagePageRefresh({
  selectedPageId,
  tabId,
  onRefreshPage,
  isOperationPending,
  isPageContentSavePending,
  setSavedPageContent,
  rootBlocks,
  assemblyBlocks,
  pageCanvasNativePreparations,
  assemblyRuntime
}: {
  selectedPageId: string | null | undefined;
  tabId: string | undefined;
  onRefreshPage: FrontStagePageProps['onRefreshPage'];
  isOperationPending: boolean;
  isPageContentSavePending: boolean;
  setSavedPageContent: Dispatch<SetStateAction<FrontstagePageContent | null>>;
  rootBlocks: FrontstageBlockInstance[];
  assemblyBlocks: FrontstageBlockInstance[];
  pageCanvasNativePreparations: ReturnType<
    typeof useFrontstagePageCanvasNativePreparations
  >;
  assemblyRuntime: ReturnType<typeof useFrontstageRuntimeAssembly>;
}) {
  const pageRefreshPendingRef = useRef(false);
  const pageRefreshGenerationRef = useRef(0);
  const [isPageRefreshPending, setIsPageRefreshPending] = useState(false);
  const [pageRefreshError, setPageRefreshError] = useState<string | null>(null);
  const handlePageRefresh = useCallback(async () => {
    if (
      !onRefreshPage ||
      pageRefreshPendingRef.current ||
      isOperationPending ||
      isPageContentSavePending
    ) {
      return;
    }

    pageRefreshPendingRef.current = true;
    const refreshGeneration = pageRefreshGenerationRef.current + 1;
    pageRefreshGenerationRef.current = refreshGeneration;
    setIsPageRefreshPending(true);
    setPageRefreshError(null);
    setSavedPageContent(null);
    try {
      await onRefreshPage();
      if (pageRefreshGenerationRef.current !== refreshGeneration) return;
      rootBlocks.forEach((block) =>
        pageCanvasNativePreparations.refreshBlock(block.id)
      );
      assemblyBlocks.forEach((block) => assemblyRuntime.refreshBlock(block.id));
    } catch (error) {
      if (pageRefreshGenerationRef.current === refreshGeneration) {
        setPageRefreshError(toDisplayErrorMessage(error));
      }
    } finally {
      if (pageRefreshGenerationRef.current === refreshGeneration) {
        pageRefreshPendingRef.current = false;
        setIsPageRefreshPending(false);
      }
    }
  }, [
    assemblyBlocks,
    assemblyRuntime,
    isOperationPending,
    isPageContentSavePending,
    onRefreshPage,
    pageCanvasNativePreparations,
    rootBlocks,
    setSavedPageContent
  ]);
  useEffect(() => {
    pageRefreshGenerationRef.current += 1;
    pageRefreshPendingRef.current = false;
    setIsPageRefreshPending(false);
    setPageRefreshError(null);
  }, [selectedPageId, tabId]);
  return { handlePageRefresh, isPageRefreshPending, pageRefreshError };
}
