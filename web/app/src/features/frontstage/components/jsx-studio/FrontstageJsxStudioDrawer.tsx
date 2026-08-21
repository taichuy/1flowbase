import type { OnMount } from '@monaco-editor/react';
import {
  createJsBlockDiagnostics,
  diagnoseLegacyBlockModuleSource,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';
import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { BlockSourceStudio } from '../../../../shared/code-block/BlockSourceStudio';
import { diagnoseUnsupportedTailwindUtilities } from '../../../../shared/code-block/tailwind-utility-diagnostics';
import { i18nText } from '../../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import { isForbiddenResponseError } from '../../lib/api-errors';
import { createFrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../lib/jsx-studio/context-injection';
import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion,
  type FrontstageJsxInsertion
} from '../../lib/jsx-studio/source-insertion';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstageBlockRuntimeDescriptor } from '../../lib/page-document';
import { createFrontstageRootNodeBlocks } from '../../lib/page-canvas/runtime-assembly';
import { findMatchingFrontstageBlockCatalogEntry } from '../../pages/frontstage-page/block-catalog-helpers';
import { FrontstageBlockCodeTabs } from './block-tabs/FrontstageBlockCodeTabs';
import type { FrontstageBlockDeletedEvent } from './block-tabs/types';
import { useFrontstageBlockTabs } from './block-tabs/use-frontstage-block-tabs';
import {
  JsxStudioResourcePanel,
  type FrontstageJsxStudioSection
} from './JsxStudioResourcePanel';
import { JsxStudioTemplatesPanel } from './JsxStudioTemplatesPanel';

export interface FrontstageJsxStudioDrawerProps {
  open: boolean;
  initialSection: FrontstageJsxStudioSection;
  workspaceId: string;
  pageId: string;
  tabId: string | null | undefined;
  block: FrontstageBlockInstance;
  pageBlocks?: readonly FrontstageBlockInstance[];
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[];
  runPanel?:
    | ReactNode
    | ((context: {
        blockId: string;
        code: string;
        runRevision: number | null;
      }) => ReactNode);
  onClose: () => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
  onSaveBlockTitle?: (blockId: string, title: string) => Promise<boolean>;
}

function blockIdFromEditorModelPath(
  modelPath: string,
  pageId: string
): string | null {
  const prefix = `file:///frontstage/${pageId}/blocks/`;
  if (!modelPath.startsWith(prefix) || !modelPath.endsWith('.tsx')) return null;
  const encodedBlockId = modelPath.slice(prefix.length, -'.tsx'.length);
  try {
    return decodeURIComponent(encodedBlockId);
  } catch {
    return null;
  }
}

export function FrontstageJsxStudioDrawer({
  block,
  catalogEntry,
  catalogEntries = [],
  initialSection,
  onClose,
  onSaveBlock,
  onSaveBlockTitle,
  open,
  pageBlocks = [],
  pageId,
  runPanel,
  tabId,
  workspaceId
}: FrontstageJsxStudioDrawerProps) {
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const [runRevision, setRunRevision] = useState<number | null>(null);
  const blockTabs = useFrontstageBlockTabs({
    workspaceId,
    pageId,
    initialBlockId: block.id,
    open
  });
  const activeTab = blockTabs.activeTab;
  const activeBlock = useMemo(
    () =>
      activeTab?.detail
        ? (createFrontstageRootNodeBlocks([activeTab.detail])[0] ?? block)
        : block,
    [activeTab?.detail, block]
  );
  const activeBlockId = activeTab?.block_id ?? block.id;
  const activeCatalogEntry = useMemo(() => {
    if (!activeTab?.detail) return catalogEntry;
    const candidates =
      catalogEntries.length > 0
        ? catalogEntries
        : catalogEntry
          ? [catalogEntry]
          : [];
    return (
      findMatchingFrontstageBlockCatalogEntry(activeBlock, candidates) ??
      (activeBlockId === block.id ? catalogEntry : null)
    );
  }, [
    activeBlock,
    activeBlockId,
    activeTab?.detail,
    block.id,
    catalogEntries,
    catalogEntry
  ]);
  const draft = activeTab?.draft ?? '';
  const permissionDenied = isForbiddenResponseError(activeTab?.error);
  const notFound =
    activeTab?.error !== null &&
    typeof activeTab?.error === 'object' &&
    activeTab.error !== null &&
    'status' in activeTab.error &&
    activeTab.error.status === 404;
  const projection = useMemo(
    () =>
      createFrontstageJsxEditorProjection({ catalogEntry: activeCatalogEntry }),
    [activeCatalogEntry]
  );
  const blockCreateDefaults = useMemo(() => {
    const template = activeCatalogEntry?.codeCapabilities?.template;
    if (!activeCatalogEntry || !template) return undefined;
    return {
      source_code: template.source,
      runtime_descriptor: createFrontstageBlockRuntimeDescriptor(activeBlock)
    };
  }, [activeBlock, activeCatalogEntry, workspaceId]);
  useEffect(() => {
    if (open) {
      setRunRevision(null);
    }
  }, [activeBlockId, open]);
  const compileDiagnostics = useMemo(() => {
    const activeTabId = activeTab?.detail?.tab_id;
    if (!activeTabId || draft.trim().length === 0) return [];
    const legacyDiagnostic = diagnoseLegacyBlockModuleSource(draft);
    if (legacyDiagnostic) {
      return createJsBlockDiagnostics(
        { pageId, tabId: activeTabId, blockId: activeBlockId },
        [legacyDiagnostic]
      );
    }
    const validation = validateNativeTrustedBlockSource(draft, {
      allowedImportSources: projection.allowedImportSources
    });
    return validation.ok
      ? createJsBlockDiagnostics(
          { pageId, tabId: activeTabId, blockId: activeBlockId },
          diagnoseUnsupportedTailwindUtilities(draft)
        )
      : createJsBlockDiagnostics(
          { pageId, tabId: activeTabId, blockId: activeBlockId },
          validation.errors
        );
  }, [
    activeBlockId,
    activeTab?.detail?.tab_id,
    draft,
    pageId,
    projection.allowedImportSources
  ]);
  const hasLegacySource = diagnoseLegacyBlockModuleSource(draft) !== null;
  const insertCode = (insertion: FrontstageJsxInsertion) => {
    const editor = editorRef.current;
    const selection = editor?.getSelection();
    const model = editor?.getModel();
    if (editor && selection && model) {
      const plan = planFrontstageJsxInsertion({
        source: model.getValue(),
        selection: {
          start: model.getOffsetAt(selection.getStartPosition()),
          end: model.getOffsetAt(selection.getEndPosition())
        },
        insertion
      });
      editor.pushUndoStop();
      editor.executeEdits(
        'frontstage-jsx-studio',
        plan.edits.map((edit) => {
          const start = model.getPositionAt(edit.start);
          const end = model.getPositionAt(edit.end);
          return {
            range: {
              startLineNumber: start.lineNumber,
              startColumn: start.column,
              endLineNumber: end.lineNumber,
              endColumn: end.column
            },
            text: edit.text,
            forceMoveMarkers: true
          };
        })
      );
      editor.pushUndoStop();
      editor.focus();
      return;
    }
    const separator = draft && !draft.endsWith('\n') ? '\n' : '';
    const source = `${draft}${separator}`;
    const plan = planFrontstageJsxInsertion({
      source,
      selection: { start: source.length, end: source.length },
      insertion
    });
    blockTabs.setActiveDraft(
      `${applyFrontstageJsxInsertionPlan(source, plan)}\n`
    );
  };
  const resolvedRunPanel =
    typeof runPanel === 'function'
      ? runPanel({ blockId: activeBlockId, code: draft, runRevision })
      : runPanel;
  const handleDeletedBlock = async (event: FrontstageBlockDeletedEvent) => {
    const result = await blockTabs.handleDeletedBlock(event);
    if (result === 'initial_root_deleted') onClose();
  };

  return (
    <BlockSourceStudio
      contextComment={projection.contextComment}
      dirty={blockTabs.anyDirty}
      errorMessage={
        notFound
          ? i18nText('frontstage', 'auto.block_tab_not_found')
          : activeTab?.error && !permissionDenied
            ? activeTab.error instanceof Error
              ? activeTab.error.message
              : i18nText('frontstage', 'auto.code_load_or_save_failed')
            : null
      }
      editorHeader={
        <FrontstageBlockCodeTabs
          activeBlockId={activeBlockId}
          initialBlockId={block.id}
          tabs={blockTabs.tabs}
          onActivate={blockTabs.activateBlock}
          onClose={blockTabs.closeBlock}
        />
      }
      extraLibs={projection.monacoExtraLibs}
      initialSection={initialSection}
      loading={activeTab?.loading ?? true}
      open={open}
      owner={`frontstage:${pageId}:${tabId ?? 'tab'}`}
      path={`file:///frontstage/${pageId}/blocks/${encodeURIComponent(activeBlockId)}.tsx`}
      readOnly={permissionDenied || notFound}
      saving={activeTab?.saving ?? false}
      sections={[
        'code',
        'templates',
        'interfaces',
        'variables',
        'block-tree',
        'components',
        'configuration',
        'run'
      ]}
      source={draft}
      testId={`frontstage-jsx-studio-${block.id}`}
      windowId={`frontstage-jsx-studio:${block.id}`}
      editorNotice={permissionDenied ? <PermissionDeniedState /> : undefined}
      editorDiagnostics={compileDiagnostics}
      onChange={(nextDraft, modelPath) => {
        if (!modelPath) {
          blockTabs.setActiveDraft(nextDraft);
          return;
        }
        const changedBlockId = blockIdFromEditorModelPath(modelPath, pageId);
        if (changedBlockId) blockTabs.setDraft(changedBlockId, nextDraft);
      }}
      onClose={onClose}
      onEditorMount={(editor) => {
        editorRef.current = editor;
      }}
      onInjectContext={injectFrontstageContextComment}
      onReset={blockTabs.resetActive}
      onRun={() => {
        if (!hasLegacySource && activeTab?.executable) {
          setRunRevision((current) => (current ?? 0) + 1);
        }
      }}
      onSave={() => {
        if (!hasLegacySource && activeTab?.executable) {
          void blockTabs.saveActiveDraft().catch(() => undefined);
        }
      }}
      renderResource={(section) =>
        section === 'templates' ? (
          <JsxStudioTemplatesPanel
            catalogEntry={activeCatalogEntry}
            readOnly={permissionDenied}
            workspaceId={workspaceId}
            onReplaceCode={blockTabs.setActiveDraft}
          />
        ) : (
          <JsxStudioResourcePanel
            block={activeBlock}
            blockCreateDefaults={blockCreateDefaults}
            codeSource={draft}
            currentBlockId={activeBlockId}
            pageBlocks={pageBlocks}
            pageId={pageId}
            tabId={activeTab?.detail?.tab_id ?? tabId}
            workspaceId={workspaceId}
            onInsertCode={insertCode}
            onDeletedBlock={(event) => void handleDeletedBlock(event)}
            onOpenBlock={blockTabs.openBlock}
            onSaveBlock={onSaveBlock}
            onSaveBlockTitle={onSaveBlockTitle}
            projection={projection}
            runPanel={resolvedRunPanel}
            section={section}
          />
        )
      }
    />
  );
}
