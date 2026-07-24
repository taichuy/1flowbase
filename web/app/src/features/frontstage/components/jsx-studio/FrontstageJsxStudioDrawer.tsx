import {
  ApiOutlined,
  AppstoreOutlined,
  CodeOutlined,
  DatabaseOutlined,
  CloseOutlined,
  CompressOutlined,
  FullscreenOutlined,
  PlayCircleOutlined,
  SettingOutlined
} from '@ant-design/icons';
import type { OnMount } from '@monaco-editor/react';
import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';
import {
  createJsBlockDiagnostics,
  validateJsBlockSource
} from '@1flowbase/page-runtime';
import { Alert, Button, Modal, Space, Tooltip, Typography } from 'antd';
import type { CSSProperties, ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { BlockSourceEditor } from '../../../../shared/code-block/BlockSourceEditor';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import {
  fitWindowWorkspaceRect,
  getWindowWorkspaceViewport
} from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import {
  WindowWorkspaceProvider,
  useOptionalWindowWorkspace,
  useWindowWorkspace
} from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import {
  closeWindowWorkspaceEntry,
  type WindowWorkspaceRect
} from '../../../../shared/ui/window-workspace/window-workspace-state';
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
import { BlockRuntimeDiagnostics } from '../BlockRuntimeDiagnostics';
import {
  JsxStudioResourcePanel,
  type FrontstageJsxStudioSection
} from './JsxStudioResourcePanel';

import './jsx-studio.css';

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
    | ((context: {
        code: string;
        onCodeChange: (code: string) => void;
      }) => ReactNode);
  onClose: () => void;
  onSaveBlock: (block: FrontstageBlockInstance) => Promise<boolean | void>;
}

const studioSections: Array<{
  key: FrontstageJsxStudioSection;
  label: string;
  icon: ReactNode;
}> = [
  {
    key: 'code',
    label: i18nText('frontstage', 'auto.code'),
    icon: <CodeOutlined />
  },
  {
    key: 'interfaces',
    label: i18nText('frontstage', 'auto.interfaces'),
    icon: <ApiOutlined />
  },
  {
    key: 'variables',
    label: i18nText('frontstage', 'auto.variables'),
    icon: <DatabaseOutlined />
  },
  {
    key: 'components',
    label: i18nText('frontstage', 'auto.components'),
    icon: <AppstoreOutlined />
  },
  {
    key: 'configuration',
    label: i18nText('frontstage', 'auto.configuration'),
    icon: <SettingOutlined />
  },
  {
    key: 'run',
    label: i18nText('frontstage', 'auto.run_preview'),
    icon: <PlayCircleOutlined />
  }
];

const DEFAULT_RESOURCE_PANEL_WIDTH = 320;
const MIN_RESOURCE_PANEL_WIDTH = 260;
const MIN_EDITOR_PANEL_WIDTH = 320;
const STUDIO_RAIL_WIDTH = 44;
const STUDIO_SPLITTER_WIDTH = 8;

export function FrontstageJsxStudioDrawer({
  ...props
}: FrontstageJsxStudioDrawerProps) {
  const sharedWindowWorkspace = useOptionalWindowWorkspace();
  if (sharedWindowWorkspace) {
    return <FrontstageJsxStudioWindow {...props} />;
  }
  return (
    <WindowWorkspaceProvider>
      <FrontstageJsxStudioWindow {...props} />
    </WindowWorkspaceProvider>
  );
}

function FrontstageJsxStudioWindow({
  block,
  pageBlocks = [],
  catalogEntry,
  diagnostics,
  initialSection,
  onClose,
  onSaveBlock,
  open,
  pageId,
  runPanel,
  tabId,
  workspaceId
}: FrontstageJsxStudioDrawerProps) {
  const windowWorkspace = useWindowWorkspace();
  const [activeSection, setActiveSection] =
    useState<FrontstageJsxStudioSection>(initialSection);
  const [mobile, setMobile] = useState(false);
  const [resourcePanelWidth, setResourcePanelWidth] = useState(
    DEFAULT_RESOURCE_PANEL_WIDTH
  );
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const liveResourcePanelWidthRef = useRef(DEFAULT_RESOURCE_PANEL_WIDTH);
  const resourcePanelDragStartRef = useRef<{
    pointerX: number;
    width: number;
  } | null>(null);
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
  const mainWindowId = `frontstage-jsx-studio:${block.codeRef}`;
  const initialWindowRect: WindowWorkspaceRect = {
    left: 120,
    top: 64,
    width: 1080,
    height: 680
  };

  useEffect(() => {
    if (!open) {
      windowWorkspace.close(mainWindowId);
      return;
    }
    windowWorkspace.open({
      id: mainWindowId,
      owner: `frontstage:${pageId}:${tabId ?? 'tab'}`,
      parent_id: null,
      rect: initialWindowRect,
      dirty
    });
    return () => windowWorkspace.close(mainWindowId);
  }, [
    mainWindowId,
    open,
    pageId,
    tabId,
    windowWorkspace.close,
    windowWorkspace.open
  ]);

  useEffect(() => {
    windowWorkspace.setDirty(mainWindowId, dirty);
  }, [dirty, mainWindowId, windowWorkspace.setDirty]);

  useEffect(() => {
    if (open) {
      setActiveSection(initialSection);
    }
  }, [initialSection, open]);

  useEffect(() => {
    const updateViewportMode = () => {
      const nextMobile = window.innerWidth <= 600;
      setMobile(nextMobile);
    };
    updateViewportMode();
    window.addEventListener('resize', updateViewportMode);
    return () => window.removeEventListener('resize', updateViewportMode);
  }, []);

  const windowEntry = windowWorkspace.state.windows.find(
    (entry) => entry.id === mainWindowId
  );
  const maxResourcePanelWidth = Math.max(
    MIN_RESOURCE_PANEL_WIDTH,
    (windowEntry?.rect.width ?? initialWindowRect.width) -
      MIN_EDITOR_PANEL_WIDTH -
      STUDIO_RAIL_WIDTH -
      STUDIO_SPLITTER_WIDTH
  );

  useEffect(() => {
    liveResourcePanelWidthRef.current = resourcePanelWidth;
  }, [resourcePanelWidth]);

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const dragStart = resourcePanelDragStartRef.current;
      if (!dragStart) return;
      setResourcePanelWidth(
        clampResourcePanelWidth(
          dragStart.width + dragStart.pointerX - event.clientX,
          maxResourcePanelWidth
        )
      );
    };
    const handleMouseUp = () => {
      resourcePanelDragStartRef.current = null;
      document.body.classList.remove('frontstage-jsx-studio--resizing-panel');
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.classList.remove('frontstage-jsx-studio--resizing-panel');
    };
  }, [maxResourcePanelWidth]);

  useEffect(() => {
    setResourcePanelWidth((current) =>
      clampResourcePanelWidth(current, maxResourcePanelWidth)
    );
  }, [maxResourcePanelWidth]);

  useEffect(() => {
    if (mobile && windowEntry && !windowEntry.maximized) {
      windowWorkspace.toggleMaximized(mainWindowId, viewportRect());
    }
  }, [mainWindowId, mobile, windowEntry, windowWorkspace.toggleMaximized]);

  const projection = useMemo(
    () =>
      createFrontstageJsxEditorProjection({
        catalogEntry
      }),
    [catalogEntry]
  );
  const allowedImports = catalogEntry?.codeCapabilities?.allowedImports ?? [];
  const compileDiagnostics = useMemo(() => {
    if (!tabId || draft.trim().length === 0) {
      return [];
    }
    const sourceValidation = validateJsBlockSource(draft, { allowedImports });
    return sourceValidation.ok
      ? []
      : createJsBlockDiagnostics(
          { pageId, tabId, blockId: block.id },
          sourceValidation.errors
        );
  }, [allowedImports, block.id, draft, pageId, tabId]);
  const selectedDiagnostics = [...diagnostics, ...compileDiagnostics].filter(
    (diagnostic) =>
      diagnostic.pageId === pageId &&
      diagnostic.tabId === tabId &&
      diagnostic.blockId === block.id
  );

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

    const separator = draft.length > 0 && !draft.endsWith('\n') ? '\n' : '';
    const source = `${draft}${separator}`;
    const plan = planFrontstageJsxInsertion({
      source,
      selection: { start: source.length, end: source.length },
      insertion
    });
    setDraft(`${applyFrontstageJsxInsertionPlan(source, plan)}\n`);
  };

  const saveCode = () => {
    void save().catch(() => undefined);
  };
  const reinjectContext = () => {
    setDraft(injectFrontstageContextComment(draft, projection.contextComment));
  };
  const statusText = loading
    ? i18nText('frontstage', 'auto.code_loading')
    : dirty
      ? i18nText('frontstage', 'auto.not_saved')
      : i18nText('frontstage', 'auto.synced');
  const resolvedRunPanel =
    typeof runPanel === 'function'
      ? runPanel({ code: draft, onCodeChange: setDraft })
      : runPanel;

  const requestClose = () => {
    const closing = closeWindowWorkspaceEntry(
      windowWorkspace.state,
      mainWindowId
    ).closed;
    const hasDirtyWindow = closing.some((entry) => entry.dirty);
    const finishClose = () => {
      windowWorkspace.close(mainWindowId);
      onClose();
    };
    if (!hasDirtyWindow) {
      finishClose();
      return;
    }
    Modal.confirm({
      title: i18nText('frontstage', 'auto.unsaved_close_title'),
      content: i18nText('frontstage', 'auto.unsaved_close_description'),
      onOk: finishClose
    });
  };
  const viewportRect = (): WindowWorkspaceRect => {
    const viewport = getWindowWorkspaceViewport();
    return fitWindowWorkspaceRect(
      {
        left: viewport.left,
        top: viewport.top,
        width: viewport.width,
        height: viewport.height
      },
      320,
      320,
      viewport
    );
  };

  if (!open) return null;
  if (!windowEntry) return null;

  return (
    <WindowWorkspaceWindow
      active={
        windowEntry.z_index ===
        Math.max(...windowWorkspace.state.windows.map((entry) => entry.z_index))
      }
      title={i18nText('frontstage', 'auto.jsx_studio')}
      testId={`frontstage-jsx-studio-${block.codeRef}`}
      className="frontstage-jsx-studio frontstage-jsx-studio--window"
      bodyClassName="frontstage-jsx-studio__drawer-body"
      dragHandleSelector="[data-window-drag-handle='true']"
      initialRect={() => windowEntry.rect}
      rect={windowEntry.rect}
      minWidth={320}
      minHeight={320}
      resizeLabel={() => i18nText('frontstage', 'auto.resize_jsx_studio')}
      zIndex={1050 + windowEntry.z_index}
      onActivate={() => windowWorkspace.activate(mainWindowId)}
      onRectChange={(nextRect) =>
        windowWorkspace.setRect(mainWindowId, nextRect)
      }
    >
      <header
        className="frontstage-jsx-studio__window-header"
        data-window-drag-handle="true"
      >
        <Space size={8}>
          <Typography.Text strong>
            {i18nText('frontstage', 'auto.jsx_studio')}
          </Typography.Text>
          <Typography.Text
            type="secondary"
            className="frontstage-jsx-studio__status"
          >
            {statusText}
          </Typography.Text>
        </Space>
        <Space className="frontstage-jsx-studio__window-actions" size={8} wrap>
          <Button onClick={reinjectContext}>
            {i18nText('frontstage', 'auto.inject_context')}
          </Button>
          <Button disabled={!dirty || loading || saving} onClick={reset}>
            {i18nText('frontstage', 'auto.reset')}
          </Button>
          <Button
            type="primary"
            disabled={!dirty || loading || saving}
            loading={saving}
            onClick={saveCode}
          >
            {i18nText('frontstage', 'auto.save_code')}
          </Button>
          <Button
            aria-label={
              windowEntry.maximized
                ? i18nText('frontstage', 'auto.restore_window')
                : i18nText('frontstage', 'auto.maximize_window')
            }
            disabled={mobile}
            icon={
              windowEntry.maximized ? (
                <CompressOutlined />
              ) : (
                <FullscreenOutlined />
              )
            }
            onClick={() =>
              windowWorkspace.toggleMaximized(mainWindowId, viewportRect())
            }
          />
          <Button
            aria-label={i18nText('frontstage', 'auto.close')}
            icon={<CloseOutlined />}
            onClick={requestClose}
          />
        </Space>
      </header>
      <div
        className={[
          'frontstage-jsx-studio__workspace',
          activeSection === 'code'
            ? 'frontstage-jsx-studio__workspace--code-only'
            : null
        ]
          .filter(Boolean)
          .join(' ')}
        style={
          {
            '--resource-panel-width': `${resourcePanelWidth}px`
          } as CSSProperties
        }
      >
        <nav
          aria-label={i18nText('frontstage', 'auto.jsx_studio_resources')}
          className="frontstage-jsx-studio__rail"
        >
          {studioSections.map((section) => (
            <Tooltip key={section.key} title={section.label} placement="left">
              <Button
                aria-label={section.label}
                className={[
                  'frontstage-jsx-studio__rail-button',
                  activeSection === section.key
                    ? 'frontstage-jsx-studio__rail-button--active'
                    : null
                ]
                  .filter(Boolean)
                  .join(' ')}
                icon={section.icon}
                type="text"
                onClick={() => setActiveSection(section.key)}
              />
            </Tooltip>
          ))}
        </nav>

        <aside
          className="frontstage-jsx-studio__resource-panel"
          style={{ display: activeSection === 'code' ? 'none' : undefined }}
        >
          <div
            style={{ display: activeSection === 'run' ? undefined : 'none' }}
          >
            {resolvedRunPanel}
          </div>
          {activeSection !== 'run' && activeSection !== 'code' ? (
            <JsxStudioResourcePanel
              block={block}
              codeSource={draft}
              pageBlocks={pageBlocks}
              workspaceId={workspaceId}
              onInsertCode={insertCode}
              onSaveBlock={onSaveBlock}
              projection={projection}
              section={activeSection}
            />
          ) : null}
        </aside>

        {activeSection !== 'code' ? (
          <div
            aria-label={i18nText('frontstage', 'auto.resize_resource_panel')}
            aria-orientation="vertical"
            aria-valuemax={maxResourcePanelWidth}
            aria-valuemin={MIN_RESOURCE_PANEL_WIDTH}
            aria-valuenow={resourcePanelWidth}
            className="frontstage-jsx-studio__panel-resize-handle"
            role="separator"
            tabIndex={0}
            onKeyDown={(event) => {
              if (event.key === 'ArrowLeft') {
                event.preventDefault();
                setResourcePanelWidth((current) =>
                  clampResourcePanelWidth(current + 40, maxResourcePanelWidth)
                );
              } else if (event.key === 'ArrowRight') {
                event.preventDefault();
                setResourcePanelWidth((current) =>
                  clampResourcePanelWidth(current - 40, maxResourcePanelWidth)
                );
              } else if (event.key === 'Home') {
                event.preventDefault();
                setResourcePanelWidth(MIN_RESOURCE_PANEL_WIDTH);
              } else if (event.key === 'End') {
                event.preventDefault();
                setResourcePanelWidth(maxResourcePanelWidth);
              }
            }}
            onMouseDown={(event) => {
              event.preventDefault();
              resourcePanelDragStartRef.current = {
                pointerX: event.clientX,
                width: liveResourcePanelWidthRef.current
              };
              document.body.classList.add(
                'frontstage-jsx-studio--resizing-panel'
              );
            }}
          />
        ) : null}

        <main className="frontstage-jsx-studio__editor-panel">
          {permissionDenied ? <PermissionDeniedState /> : null}
          {error && !permissionDenied ? (
            <Alert
              type="error"
              showIcon
              message={i18nText('frontstage', 'auto.code_load_or_save_failed')}
            />
          ) : null}
          <div className="frontstage-jsx-studio__monaco">
            <BlockSourceEditor
              ariaLabel={i18nText('frontstage', 'auto.code')}
              extraLibs={projection.monacoExtraLibs}
              height="100%"
              path={`file:///frontstage/${pageId}/${tabId ?? 'tab'}/${block.id}.tsx`}
              value={draft}
              onMount={(editor) => {
                editorRef.current = editor;
              }}
              onChange={setDraft}
              readOnly={loading || saving || permissionDenied}
            />
          </div>
          <div className="frontstage-jsx-studio__problems">
            <BlockRuntimeDiagnostics diagnostics={selectedDiagnostics} />
          </div>
        </main>
      </div>
    </WindowWorkspaceWindow>
  );
}

function clampResourcePanelWidth(width: number, maxWidth: number) {
  return Math.min(maxWidth, Math.max(MIN_RESOURCE_PANEL_WIDTH, width));
}
