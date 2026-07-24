import type { OnMount } from '@monaco-editor/react';
import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';
import { Alert, Button, Modal } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';

import { i18nText } from '../i18n/text';
import { WindowWorkspaceWindow } from '../ui/window-workspace/WindowWorkspaceWindow';
import {
  fitWindowWorkspaceRect,
  getWindowWorkspaceViewport
} from '../ui/window-workspace/window-workspace-geometry';
import {
  WindowWorkspaceProvider,
  useOptionalWindowWorkspace,
  useWindowWorkspace
} from '../ui/window-workspace/WindowWorkspaceProvider';
import {
  closeWindowWorkspaceEntry,
  type WindowWorkspaceRect
} from '../ui/window-workspace/window-workspace-state';
import { BlockSourceEditor } from './BlockSourceEditor';
import {
  BlockStudioWorkspace,
  type BlockStudioSection
} from './BlockStudioWorkspace';
import { BlockStudioWindowHeader } from './BlockStudioWindowHeader';

import './block-source-studio.css';

export interface BlockSourceStudioProps {
  contextComment: string;
  dirty: boolean;
  editorFooter?: ReactNode;
  editorNotice?: ReactNode;
  errorMessage?: string | null;
  extraLibs?: readonly FrontendBlockMonacoExtraLib[];
  initialSection: BlockStudioSection;
  loading: boolean;
  open: boolean;
  owner: string;
  path: string;
  readOnly: boolean;
  saving: boolean;
  source: string;
  testId: string;
  windowId: string;
  onChange: (source: string) => void;
  onClose: () => void;
  onEditorMount?: OnMount;
  onInjectContext: (source: string, contextComment: string) => string;
  onReset: () => void;
  onSave: () => void;
  renderResource: (
    section: Exclude<BlockStudioSection, 'code'>
  ) => ReactNode;
}

const INITIAL_WINDOW_RECT: WindowWorkspaceRect = {
  left: 120,
  top: 64,
  width: 1080,
  height: 680
};

export function BlockSourceStudio(props: BlockSourceStudioProps) {
  const sharedWindowWorkspace = useOptionalWindowWorkspace();
  if (sharedWindowWorkspace) return <BlockSourceStudioWindow {...props} />;
  return (
    <WindowWorkspaceProvider>
      <BlockSourceStudioWindow {...props} />
    </WindowWorkspaceProvider>
  );
}

function BlockSourceStudioWindow({
  contextComment,
  dirty,
  editorFooter,
  editorNotice,
  errorMessage,
  extraLibs = [],
  initialSection,
  loading,
  onChange,
  onClose,
  onEditorMount,
  onInjectContext,
  onReset,
  onSave,
  open,
  owner,
  path,
  readOnly,
  renderResource,
  saving,
  source,
  testId,
  windowId
}: BlockSourceStudioProps) {
  const {
    activate,
    close,
    open: openWindow,
    setDirty,
    setRect,
    state: windowWorkspaceState,
    toggleMaximized
  } = useWindowWorkspace();
  const [mobile, setMobile] = useState(false);

  useEffect(() => {
    if (!open) {
      close(windowId);
      return;
    }
    openWindow({
      id: windowId,
      owner,
      parent_id: null,
      rect: INITIAL_WINDOW_RECT,
      dirty: false
    });
    return () => close(windowId);
  }, [close, open, openWindow, owner, windowId]);

  useEffect(() => {
    setDirty(windowId, dirty);
  }, [dirty, setDirty, windowId]);

  useEffect(() => {
    const updateViewportMode = () => setMobile(window.innerWidth <= 600);
    updateViewportMode();
    window.addEventListener('resize', updateViewportMode);
    return () => window.removeEventListener('resize', updateViewportMode);
  }, []);

  const windowEntry = windowWorkspaceState.windows.find(
    (entry) => entry.id === windowId
  );

  useEffect(() => {
    if (mobile && windowEntry && !windowEntry.maximized) {
      toggleMaximized(windowId, viewportRect());
    }
  }, [mobile, toggleMaximized, windowEntry, windowId]);

  const requestClose = () => {
    const closing = closeWindowWorkspaceEntry(
      windowWorkspaceState,
      windowId
    ).closed;
    const finishClose = () => {
      close(windowId);
      onClose();
    };
    if (!closing.some((entry) => entry.dirty)) {
      finishClose();
      return;
    }
    Modal.confirm({
      title: i18nText('frontstage', 'auto.unsaved_close_title'),
      content: i18nText('frontstage', 'auto.unsaved_close_description'),
      onOk: finishClose
    });
  };

  if (!open || !windowEntry) return null;
  const status = loading
    ? i18nText('frontstage', 'auto.code_loading')
    : dirty
      ? i18nText('frontstage', 'auto.not_saved')
      : i18nText('frontstage', 'auto.synced');

  return (
    <WindowWorkspaceWindow
      active={
        windowEntry.z_index ===
        Math.max(...windowWorkspaceState.windows.map((entry) => entry.z_index))
      }
      bodyClassName="frontstage-jsx-studio__window-body"
      className="frontstage-jsx-studio frontstage-jsx-studio--window"
      dragHandleSelector="[data-window-drag-handle='true']"
      initialRect={() => windowEntry.rect}
      minHeight={320}
      minWidth={320}
      rect={windowEntry.rect}
      resizeLabel={() => i18nText('frontstage', 'auto.resize_jsx_studio')}
      testId={testId}
      title={i18nText('frontstage', 'auto.jsx_studio')}
      zIndex={1050 + windowEntry.z_index}
      onActivate={() => activate(windowId)}
      onRectChange={(nextRect) => setRect(windowId, nextRect)}
    >
      <BlockStudioWindowHeader
        closeLabel={i18nText('frontstage', 'auto.close')}
        maximized={windowEntry.maximized}
        maximizeLabel={i18nText('frontstage', 'auto.maximize_window')}
        mobile={mobile}
        restoreLabel={i18nText('frontstage', 'auto.restore_window')}
        status={status}
        title={i18nText('frontstage', 'auto.jsx_studio')}
        toolbar={(
          <>
            <Button
              onClick={() => onChange(onInjectContext(source, contextComment))}
            >
              {i18nText('frontstage', 'auto.inject_context')}
            </Button>
            <Button disabled={!dirty || loading || saving} onClick={onReset}>
              {i18nText('frontstage', 'auto.reset')}
            </Button>
            <Button
              disabled={!dirty || loading || readOnly || saving}
              loading={saving}
              type="primary"
              onClick={onSave}
            >
              {i18nText('frontstage', 'auto.save_code')}
            </Button>
          </>
        )}
        onClose={requestClose}
        onToggleMaximized={() =>
          toggleMaximized(windowId, viewportRect())
        }
      />
      <BlockStudioWorkspace
        initialSection={initialSection}
        open={open}
        renderResource={renderResource}
        windowWidth={windowEntry.rect.width}
        editor={(
          <main className="frontstage-jsx-studio__editor-panel">
            {editorNotice}
            {errorMessage ? (
              <Alert message={errorMessage} showIcon type="error" />
            ) : null}
            <div className="frontstage-jsx-studio__monaco">
              <BlockSourceEditor
                ariaLabel={i18nText('frontstage', 'auto.code')}
                extraLibs={extraLibs}
                height="100%"
                path={path}
                readOnly={loading || readOnly || saving}
                value={source}
                onChange={onChange}
                onMount={onEditorMount}
              />
            </div>
            {editorFooter}
          </main>
        )}
      />
    </WindowWorkspaceWindow>
  );
}

function viewportRect(): WindowWorkspaceRect {
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
}
