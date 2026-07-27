import type { OnMount } from '@monaco-editor/react';
import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';
import {
  createJsBlockDiagnostics,
  diagnoseLegacyBlockModuleSource,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';
import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { BlockSourceStudio } from '../../../../shared/code-block/BlockSourceStudio';
import { i18nText } from '../../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import { useFrontstageBlockCode } from '../../hooks/use-frontstage-block-code';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import { createFrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../lib/jsx-studio/context-injection';
import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion,
  type FrontstageJsxInsertion
} from '../../lib/jsx-studio/source-insertion';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  JsxStudioResourcePanel,
  type FrontstageJsxStudioSection
} from './JsxStudioResourcePanel';

export interface FrontstageJsxStudioDrawerProps {
  open: boolean;
  initialSection: FrontstageJsxStudioSection;
  workspaceId: string;
  pageId: string;
  tabId: string | null | undefined;
  block: FrontstageBlockInstance;
  pageBlocks?: readonly FrontstageBlockInstance[];
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  diagnostics: BlockRuntimeDiagnostic[];
  runPanel?:
    | ReactNode
    | ((context: { code: string; runRevision: number | null }) => ReactNode);
  onClose: () => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
}

export function FrontstageJsxStudioDrawer({
  block,
  catalogEntry,
  initialSection,
  onClose,
  onSaveBlock,
  open,
  pageBlocks = [],
  pageId,
  runPanel,
  tabId,
  workspaceId
}: FrontstageJsxStudioDrawerProps) {
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const [runRevision, setRunRevision] = useState<number | null>(null);
  const {
    draft,
    dirty,
    error,
    loading,
    permissionDenied,
    reset,
    save,
    saving,
    setDraft
  } = useFrontstageBlockCode({
    workspaceId,
    pageId,
    codeRef: block.codeRef
  });
  const projection = useMemo(
    () => createFrontstageJsxEditorProjection({ catalogEntry }),
    [catalogEntry]
  );
  useEffect(() => {
    if (open) setRunRevision(null);
  }, [block.id, open]);
  const compileDiagnostics = useMemo(() => {
    if (!tabId || draft.trim().length === 0) return [];
    const legacyDiagnostic = diagnoseLegacyBlockModuleSource(draft);
    if (legacyDiagnostic) {
      return createJsBlockDiagnostics(
        { pageId, tabId, blockId: block.id },
        [legacyDiagnostic]
      );
    }
    const validation = validateNativeTrustedBlockSource(draft, {
      allowedImportSources: projection.allowedImportSources
    });
    return validation.ok
      ? []
      : createJsBlockDiagnostics(
          { pageId, tabId, blockId: block.id },
          validation.errors
        );
  }, [block.id, draft, pageId, projection.allowedImportSources, tabId]);
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
    setDraft(`${applyFrontstageJsxInsertionPlan(source, plan)}\n`);
  };
  const resolvedRunPanel =
    typeof runPanel === 'function'
      ? runPanel({ code: draft, runRevision })
      : runPanel;

  return (
    <BlockSourceStudio
      contextComment={projection.contextComment}
      dirty={dirty}
      errorMessage={
        error && !permissionDenied
          ? i18nText('frontstage', 'auto.code_load_or_save_failed')
          : null
      }
      extraLibs={projection.monacoExtraLibs}
      initialSection={initialSection}
      loading={loading}
      open={open}
      owner={`frontstage:${pageId}:${tabId ?? 'tab'}`}
      path={`file:///frontstage/${pageId}/${tabId ?? 'tab'}/${block.id}.tsx`}
      readOnly={permissionDenied}
      saving={saving}
      source={draft}
      testId={`frontstage-jsx-studio-${block.codeRef}`}
      windowId={`frontstage-jsx-studio:${block.codeRef}`}
      editorNotice={permissionDenied ? <PermissionDeniedState /> : null}
      editorDiagnostics={compileDiagnostics}
      onChange={setDraft}
      onClose={onClose}
      onEditorMount={(editor) => {
        editorRef.current = editor;
      }}
      onInjectContext={injectFrontstageContextComment}
      onReset={reset}
      onRun={() => {
        if (!hasLegacySource) {
          setRunRevision((current) => (current ?? 0) + 1);
        }
      }}
      onSave={() => {
        if (!hasLegacySource) void save().catch(() => undefined);
      }}
      renderResource={(section) => (
        <JsxStudioResourcePanel
          block={block}
          codeSource={draft}
          pageBlocks={pageBlocks}
          workspaceId={workspaceId}
          onInsertCode={insertCode}
          onSaveBlock={onSaveBlock}
          projection={projection}
          runPanel={resolvedRunPanel}
          section={section}
        />
      )}
    />
  );
}
