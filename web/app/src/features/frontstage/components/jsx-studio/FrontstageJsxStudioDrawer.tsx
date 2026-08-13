import type { OnMount } from '@monaco-editor/react';
import { Alert, Button, Modal, Space } from 'antd';
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
import {
  resolveFrontstageNativeDependencyLock,
  type NormalizedFrontstageBlockCatalogEntry
} from '../../lib/block-catalog';
import { isForbiddenResponseError } from '../../lib/api-errors';
import { createFrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../lib/jsx-studio/context-injection';
import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion,
  type FrontstageJsxInsertion
} from '../../lib/jsx-studio/source-insertion';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { FrontstageBlockCodeTabs } from './block-tabs/FrontstageBlockCodeTabs';
import type { FrontstageBlockDeletedEvent } from './block-tabs/types';
import {
  useFrontstageBlockTabs,
  type FrontstageExecutableSavePayload
} from './block-tabs/use-frontstage-block-tabs';
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
  runPanel?:
    | ReactNode
    | ((context: {
        blockId: string;
        code: string;
        runRevision: number | null;
      }) => ReactNode);
  onClose: () => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
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
  const [executablePreview, setExecutablePreview] = useState<{
    block_id: string;
    source_code: string;
    kind: 'migration' | 'upgrade';
    payload: FrontstageExecutableSavePayload;
  } | null>(null);
  const [executableOperationError, setExecutableOperationError] = useState<
    string | null
  >(null);
  const blockTabs = useFrontstageBlockTabs({
    workspaceId,
    pageId,
    initialBlockId: block.id,
    open
  });
  const activeTab = blockTabs.activeTab;
  const activeBlockId = activeTab?.block_id ?? block.id;
  const draft = activeTab?.draft ?? '';
  const permissionDenied = isForbiddenResponseError(activeTab?.error);
  const notFound =
    activeTab?.error !== null &&
    typeof activeTab?.error === 'object' &&
    activeTab.error !== null &&
    'status' in activeTab.error &&
    activeTab.error.status === 404;
  const projection = useMemo(
    () => createFrontstageJsxEditorProjection({ catalogEntry }),
    [catalogEntry]
  );
  useEffect(() => {
    if (open) {
      setRunRevision(null);
      setExecutablePreview(null);
      setExecutableOperationError(null);
    }
  }, [activeBlockId, open]);
  useEffect(() => {
    setExecutablePreview(null);
    setExecutableOperationError(null);
  }, [draft]);
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
  const dependencyLockResolution = useMemo(
    () => resolveFrontstageNativeDependencyLock({ catalogEntry, workspaceId }),
    [catalogEntry, workspaceId]
  );
  const executableState = activeTab?.executable?.executable_state;
  const previewKind = executableState === 'legacy' ? 'migration' : 'upgrade';
  const previewMatchesDraft =
    executablePreview?.block_id === activeBlockId &&
    executablePreview.source_code === draft &&
    executablePreview.kind === previewKind;

  const previewCurrentDependencies = async () => {
    setExecutableOperationError(null);
    if (compileDiagnostics.length > 0) {
      setExecutableOperationError(
        i18nText('frontstage', 'auto.executable_preview_validation_failed')
      );
      return;
    }
    if (dependencyLockResolution.error) {
      setExecutableOperationError(dependencyLockResolution.error);
      return;
    }
    try {
      const payload = await blockTabs.previewActive(
        'upgrade',
        dependencyLockResolution.dependencyLock
      );
      setExecutablePreview({
        block_id: activeBlockId,
        source_code: draft,
        kind: previewKind,
        payload
      });
      setRunRevision((current) => (current ?? 0) + 1);
    } catch (error) {
      setExecutableOperationError(
        error instanceof Error
          ? error.message
          : i18nText('frontstage', 'auto.executable_preview_failed')
      );
    }
  };

  const applyCurrentDependencies = () => {
    if (!executablePreview || !previewMatchesDraft) return;
    const kind = executablePreview.kind;
    Modal.confirm({
      title: i18nText(
        'frontstage',
        kind === 'migration'
          ? 'auto.confirm_legacy_migration'
          : 'auto.confirm_dependency_upgrade'
      ),
      content: i18nText(
        'frontstage',
        kind === 'migration'
          ? 'auto.confirm_legacy_migration_description'
          : 'auto.confirm_dependency_upgrade_description'
      ),
      onOk: async () => {
        try {
          await blockTabs.saveActive(executablePreview.payload);
          setExecutablePreview(null);
          setExecutableOperationError(null);
        } catch (error) {
          setExecutableOperationError(
            error instanceof Error
              ? error.message
              : i18nText('frontstage', 'auto.executable_apply_failed')
          );
          throw error;
        }
      }
    });
  };

  const saveReadyDraft = async () => {
    setExecutableOperationError(null);
    if (compileDiagnostics.length > 0) {
      setExecutableOperationError(
        i18nText('frontstage', 'auto.executable_preview_validation_failed')
      );
      return;
    }
    try {
      const payload = await blockTabs.previewActive('preserve');
      await blockTabs.saveActive(payload);
    } catch (error) {
      setExecutableOperationError(
        error instanceof Error
          ? error.message
          : i18nText('frontstage', 'auto.executable_apply_failed')
      );
    }
  };
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
      editorNotice={
        permissionDenied ? (
          <PermissionDeniedState />
        ) : (
          <Alert
            action={
              <Space wrap>
                <Button
                  disabled={activeTab?.loading || activeTab?.saving}
                  size="small"
                  onClick={() => void previewCurrentDependencies()}
                >
                  {i18nText(
                    'frontstage',
                    executableState === 'legacy'
                      ? 'auto.preview_legacy_migration'
                      : 'auto.preview_dependency_upgrade'
                  )}
                </Button>
                {previewMatchesDraft ? (
                  <Button
                    disabled={activeTab?.saving}
                    size="small"
                    type="primary"
                    onClick={applyCurrentDependencies}
                  >
                    {i18nText(
                      'frontstage',
                      previewKind === 'migration'
                        ? 'auto.apply_legacy_migration'
                        : 'auto.apply_dependency_upgrade'
                    )}
                  </Button>
                ) : null}
              </Space>
            }
            description={
              executableOperationError ??
              (previewMatchesDraft
                ? i18nText('frontstage', 'auto.executable_preview_ready')
                : executableState === 'legacy'
                  ? i18nText('frontstage', 'auto.legacy_migration_description')
                  : i18nText(
                      'frontstage',
                      'auto.dependency_upgrade_description'
                    ))
            }
            message={i18nText(
              'frontstage',
              executableState === 'legacy'
                ? 'auto.legacy_executable_state'
                : 'auto.executable_dependency_state'
            )}
            showIcon
            type={
              executableOperationError
                ? 'error'
                : previewMatchesDraft
                  ? 'success'
                  : 'info'
            }
          />
        )
      }
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
        if (!hasLegacySource) {
          setRunRevision((current) => (current ?? 0) + 1);
        }
      }}
      onSave={() => {
        if (!hasLegacySource && executableState === 'ready') {
          void saveReadyDraft();
        }
      }}
      renderResource={(section) =>
        section === 'templates' ? (
          <JsxStudioTemplatesPanel
            catalogEntry={catalogEntry}
            readOnly={permissionDenied}
            workspaceId={workspaceId}
            onReplaceCode={blockTabs.setActiveDraft}
          />
        ) : (
          <JsxStudioResourcePanel
            block={block}
            codeSource={draft}
            currentBlockId={activeBlockId}
            pageBlocks={pageBlocks}
            pageId={pageId}
            workspaceId={workspaceId}
            onInsertCode={insertCode}
            onDeletedBlock={(event) => void handleDeletedBlock(event)}
            onOpenBlock={blockTabs.openBlock}
            onSaveBlock={onSaveBlock}
            projection={projection}
            runPanel={resolvedRunPanel}
            section={section}
          />
        )
      }
    />
  );
}
