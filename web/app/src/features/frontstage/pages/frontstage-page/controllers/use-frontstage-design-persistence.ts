import { useCallback, useRef, useState } from 'react';

import type { FrontstagePageContent } from '../../../api/page-content';

import { useFrontstageBlockCatalog } from '../../../hooks/use-frontstage-block-catalog';

import { useFrontstageBlockTreeMutations } from '../../../hooks/use-frontstage-block-tree-mutations';
import { useFrontstagePageContentSave } from '../../../hooks/use-frontstage-page-content-save';
import {
  appendFrontstageBlock,
  type FrontstageBlockCompositionState
} from '../../../lib/block-composition';

import {
  createFrontstageBlockRuntimeDescriptor,
  type FrontstageBlockInstance
} from '../../../lib/page-document';

import { i18nText } from '../../../../../shared/i18n/text';
import { createCatalogBlockInput } from '../block-catalog-helpers';
import { toDisplayErrorMessage } from '../page-action-helpers';

import type { Dispatch, SetStateAction } from 'react';

/** Owns design write transactions, pending guards and saved-content/error receipts. */
export function useFrontstageDesignPersistence({
  workspaceId,
  selectedPageId,
  tabId,
  setSelectedBlockId
}: {
  workspaceId: string;
  selectedPageId: string | null | undefined;
  tabId: string | undefined;
  setSelectedBlockId: Dispatch<SetStateAction<string | null>>;
}) {
  const [savedPageContent, setSavedPageContent] =
    useState<FrontstagePageContent | null>(null);
  const [isBlockSavePending, setIsBlockSavePending] = useState(false);
  const blockCreationPendingRef = useRef(false);
  const [blockSaveError, setBlockSaveError] = useState<string | null>(null);
  const pageContentSave = useFrontstagePageContentSave({
    workspaceId,
    pageId: selectedPageId,
    tabId
  });
  const blockTreeMutations = useFrontstageBlockTreeMutations(
    workspaceId,
    selectedPageId ?? ''
  );
  const saveBlockComposition = useCallback(
    async (
      sourceContent: FrontstagePageContent,
      compositionState: FrontstageBlockCompositionState
    ) => {
      setIsBlockSavePending(true);
      setBlockSaveError(null);
      pageContentSave.clearError();

      try {
        if (!tabId) throw new Error('missing frontstage tab id');
        if (compositionState.document.blocks.length > 0) {
          await blockTreeMutations.updateDescriptors.mutateAsync({
            tab_id: tabId,
            input: {
              updates: compositionState.document.blocks.map((block) => ({
                block_id: block.id,
                runtime_descriptor:
                  createFrontstageBlockRuntimeDescriptor(block)
              }))
            }
          });
        }
        const documentPayload = sourceContent.document.payload;
        const documentRecord: Record<string, unknown> =
          typeof documentPayload === 'object' &&
          documentPayload !== null &&
          !Array.isArray(documentPayload)
            ? (documentPayload as Record<string, unknown>)
            : {};
        const nextContent = await pageContentSave.save({
          payload: {
            ...documentRecord,
            'x-layout-mode': compositionState.document.layoutMode
          }
        });
        setSavedPageContent(nextContent);
        setSelectedBlockId(compositionState.selectedBlockId);
        return true;
      } catch (error) {
        setBlockSaveError(toDisplayErrorMessage(error));
        return false;
      } finally {
        setIsBlockSavePending(false);
      }
    },
    [
      blockTreeMutations.updateDescriptors,
      pageContentSave,
      tabId,
      setSelectedBlockId
    ]
  );

  const saveStudioBlock = useCallback(
    async (nextBlock: FrontstageBlockInstance) => {
      if (!selectedPageId) {
        return false;
      }
      try {
        await blockTreeMutations.update.mutateAsync({
          block_id: nextBlock.id,
          input: {
            runtime_descriptor:
              createFrontstageBlockRuntimeDescriptor(nextBlock)
          }
        });
        return true;
      } catch (error) {
        setBlockSaveError(toDisplayErrorMessage(error));
        return false;
      }
    },
    [blockTreeMutations.update, selectedPageId]
  );
  const saveStudioBlockTitle = useCallback(
    async (blockId: string, title: string) => {
      if (!selectedPageId) return false;
      try {
        await blockTreeMutations.update.mutateAsync({
          block_id: blockId,
          input: { title }
        });
        return true;
      } catch (error) {
        setBlockSaveError(toDisplayErrorMessage(error));
        return false;
      }
    },
    [blockTreeMutations.update, selectedPageId]
  );
  const addBlock = async ({
    canAddBlock,
    blockCatalog,
    activePageContent,
    blockCompositionState
  }: {
    canAddBlock: boolean;
    blockCatalog: ReturnType<typeof useFrontstageBlockCatalog>;
    activePageContent: FrontstagePageContent | undefined;
    blockCompositionState: FrontstageBlockCompositionState | null;
  }) => {
    if (
      !canAddBlock ||
      blockCatalog.loading ||
      blockCreationPendingRef.current
    ) {
      return;
    }
    setBlockSaveError(null);
    pageContentSave.clearError();
    if (!activePageContent || !blockCompositionState || !tabId) {
      return;
    }

    if (blockCatalog.error) {
      setBlockSaveError(toDisplayErrorMessage(blockCatalog.error));
      return;
    }

    if (blockCatalog.items.length === 0) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.no_available_block_catalog_entries')
      );
      return;
    }

    if (blockCatalog.items.length !== 1) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.block_catalog_load_failed')
      );
      return;
    }

    const [entry] = blockCatalog.items;
    if (!entry) return;

    const codeTemplate = entry.codeCapabilities?.template;
    if (!codeTemplate) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.catalog_entry_missing_code_template')
      );
      return;
    }

    const nextBlockInput = createCatalogBlockInput(
      entry,
      blockCompositionState.document.blocks.length
    );
    const nextCompositionState = appendFrontstageBlock(
      blockCompositionState,
      nextBlockInput
    );

    blockCreationPendingRef.current = true;
    setIsBlockSavePending(true);
    try {
      const createdBlock =
        nextCompositionState.document.blocks.find(
          (block) => block.id === nextCompositionState.selectedBlockId
        ) ??
        nextCompositionState.document.blocks[
          nextCompositionState.document.blocks.length - 1
        ];
      if (!createdBlock) {
        throw new Error('created block is missing');
      }

      const createdNode = await blockTreeMutations.create.mutateAsync({
        tab_id: tabId,
        description: '',
        presentation: 'page',
        parent_block_id: null,
        before_block_id: null,
        after_block_id: null,
        source_code: codeTemplate.source,
        runtime_descriptor: createFrontstageBlockRuntimeDescriptor(createdBlock)
      });
      setSelectedBlockId(createdNode.block_id);
    } catch (error) {
      setBlockSaveError(toDisplayErrorMessage(error));
    } finally {
      blockCreationPendingRef.current = false;
      setIsBlockSavePending(false);
    }
  };

  return {
    savedPageContent,
    setSavedPageContent,
    isBlockSavePending,
    blockSaveError,
    setBlockSaveError,
    pageContentSave,
    blockTreeMutations,
    saveBlockComposition,
    saveStudioBlock,
    saveStudioBlockTitle,
    addBlock
  };
}
