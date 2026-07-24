import { CodeOutlined } from '@ant-design/icons';
import { Alert, Button, Modal, Tooltip } from 'antd';
import { useEffect, useState } from 'react';

import { BlockSourceEditor } from '../../../../shared/code-block/BlockSourceEditor';
import { BlockStudioWindowHeader } from '../../../../shared/code-block/BlockStudioWindowHeader';
import { i18nText } from '../../../../shared/i18n/text';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import {
  fitWindowWorkspaceRect,
  getWindowWorkspaceViewport
} from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import {
  closeWindowWorkspaceEntry,
  type WindowWorkspaceRect
} from '../../../../shared/ui/window-workspace/window-workspace-state';

import '../../../../shared/code-block/block-source-studio.css';

export interface AuthenticatorUiBlockStudioProps {
  authenticatorId: string;
  errorMessage: string | null;
  open: boolean;
  readOnly: boolean;
  saving: boolean;
  source: string;
  onClose: () => void;
  onSave: (source: string) => Promise<void>;
}

const INITIAL_WINDOW_RECT: WindowWorkspaceRect = {
  left: 120,
  top: 64,
  width: 1080,
  height: 680
};

export function AuthenticatorUiBlockStudio({
  authenticatorId,
  errorMessage,
  onClose,
  onSave,
  open,
  readOnly,
  saving,
  source
}: AuthenticatorUiBlockStudioProps) {
  const {
    activate,
    close,
    open: openWindow,
    setDirty,
    setRect,
    state: windowWorkspaceState,
    toggleMaximized
  } = useWindowWorkspace();
  const [draft, setDraft] = useState(source);
  const [mobile, setMobile] = useState(false);
  const dirty = draft !== source;
  const windowId = `auth-center-jsx-studio:${authenticatorId}`;

  useEffect(() => {
    if (!open) {
      close(windowId);
      return;
    }
    setDraft(source);
    openWindow({
      id: windowId,
      owner: 'settings:auth-center',
      parent_id: null,
      rect: INITIAL_WINDOW_RECT,
      dirty: false
    });
    return () => close(windowId);
  }, [close, open, openWindow, source, windowId]);

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
      testId={`auth-center-jsx-studio-${authenticatorId}`}
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
        status={
          dirty
            ? i18nText('frontstage', 'auto.not_saved')
            : i18nText('frontstage', 'auto.synced')
        }
        title={i18nText('frontstage', 'auto.jsx_studio')}
        toolbar={(
          <>
            <Button
              disabled={!dirty || saving}
              onClick={() => setDraft(source)}
            >
              {i18nText('frontstage', 'auto.reset')}
            </Button>
            <Button
              disabled={!dirty || readOnly || saving}
              loading={saving}
              type="primary"
              onClick={() => void onSave(draft)}
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
      <div className="frontstage-jsx-studio__workspace frontstage-jsx-studio__workspace--code-only">
        <main className="frontstage-jsx-studio__editor-panel">
          {errorMessage ? (
            <Alert message={errorMessage} showIcon type="error" />
          ) : null}
          <div className="frontstage-jsx-studio__monaco">
            <BlockSourceEditor
              ariaLabel={i18nText('settings', 'auto.auth_center_block_source')}
              height="100%"
              path={`file:///auth-center/${authenticatorId}/public-ui-block.tsx`}
              readOnly={readOnly || saving}
              value={draft}
              onChange={setDraft}
            />
          </div>
        </main>
        <nav
          aria-label={i18nText('frontstage', 'auto.jsx_studio_resources')}
          className="frontstage-jsx-studio__rail"
        >
          <Tooltip title={i18nText('frontstage', 'auto.code')} placement="left">
            <Button
              aria-label={i18nText('frontstage', 'auto.code')}
              className="frontstage-jsx-studio__rail-button frontstage-jsx-studio__rail-button--active"
              icon={<CodeOutlined />}
              type="text"
            />
          </Tooltip>
        </nav>
      </div>
    </WindowWorkspaceWindow>
  );
}
