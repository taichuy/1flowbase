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
import Editor, { type BeforeMount, type OnMount } from '@monaco-editor/react';
import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';
import {
  createJsBlockDiagnostics,
  validateJsBlockSource
} from '@1flowbase/page-runtime';
import { Alert, Button, Modal, Space, Tooltip, Typography } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';
import { useFrontstageBlockCode } from '../../hooks/use-frontstage-block-code';
import { useFrontstageCallableInterfaces } from '../../hooks/use-frontstage-callable-interfaces';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import { createFrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../lib/jsx-studio/context-injection';
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

export function FrontstageJsxStudioDrawer({
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
  const [activeSection, setActiveSection] =
    useState<FrontstageJsxStudioSection>(initialSection);
  const [windowRect, setWindowRect] = useState<WindowWorkspaceRect>(() => ({
    left: 120,
    top: 64,
    width: 1080,
    height: 760
  }));
  const [maximized, setMaximized] = useState(false);
  const [mobile, setMobile] = useState(false);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const callableInterfaces = useFrontstageCallableInterfaces(workspaceId);
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

  useEffect(() => {
    if (open) {
      setActiveSection(initialSection);
    }
  }, [initialSection, open]);

  useEffect(() => {
    const updateViewportMode = () => {
      const nextMobile = window.innerWidth <= 600;
      setMobile(nextMobile);
      if (nextMobile) setMaximized(true);
    };
    updateViewportMode();
    window.addEventListener('resize', updateViewportMode);
    return () => window.removeEventListener('resize', updateViewportMode);
  }, []);

  const projection = useMemo(
    () =>
      createFrontstageJsxEditorProjection({
        block,
        catalogEntry,
        callableInterfaces: callableInterfaces.data
      }),
    [block, callableInterfaces.data, catalogEntry]
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

  const configureMonaco: BeforeMount = (monaco) => {
    monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
      allowNonTsExtensions: true,
      jsx: monaco.languages.typescript.JsxEmit.ReactJSX,
      moduleResolution: monaco.languages.typescript.ModuleResolutionKind.NodeJs,
      target: monaco.languages.typescript.ScriptTarget.ES2022
    });
    projection.monacoExtraLibs.forEach((extraLib) => {
      monaco.languages.typescript.typescriptDefaults.addExtraLib(
        extraLib.content,
        extraLib.filePath
      );
    });
  };

  const insertCode = (source: string) => {
    const editor = editorRef.current;
    const selection = editor?.getSelection();
    if (editor && selection) {
      editor.executeEdits('frontstage-jsx-studio', [
        {
          range: selection,
          text: source,
          forceMoveMarkers: true
        }
      ]);
      editor.focus();
      return;
    }

    const separator = draft.length > 0 && !draft.endsWith('\n') ? '\n' : '';
    setDraft(`${draft}${separator}${source}\n`);
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
    if (!dirty) {
      onClose();
      return;
    }
    Modal.confirm({
      title: i18nText('frontstage', 'auto.unsaved_close_title'),
      content: i18nText('frontstage', 'auto.unsaved_close_description'),
      onOk: onClose
    });
  };
  const viewportRect = (): WindowWorkspaceRect => ({
    left: 8,
    top: 8,
    width: Math.max(320, window.innerWidth - 16),
    height: Math.max(320, window.innerHeight - 16)
  });

  if (!open) return null;

  return (
    <WindowWorkspaceWindow
      active
      title={i18nText('frontstage', 'auto.jsx_studio')}
      testId={`frontstage-jsx-studio-${block.codeRef}`}
      className="frontstage-jsx-studio frontstage-jsx-studio--window"
      bodyClassName="frontstage-jsx-studio__drawer-body"
      dragHandleSelector="[data-window-drag-handle='true']"
      initialRect={() => (mobile ? viewportRect() : windowRect)}
      rect={maximized ? viewportRect() : windowRect}
      minWidth={320}
      minHeight={320}
      resizeLabel={() => i18nText('frontstage', 'auto.resize_jsx_studio')}
      onActivate={() => undefined}
      onRectChange={(nextRect) => {
        if (!maximized) setWindowRect(nextRect);
      }}
    >
      <header
        className="frontstage-jsx-studio__window-header"
        data-window-drag-handle="true"
      >
        <Typography.Text strong>
          {i18nText('frontstage', 'auto.jsx_studio')}
        </Typography.Text>
        <Space size={8}>
          <Typography.Text
            type="secondary"
            className="frontstage-jsx-studio__status"
          >
            {statusText}
          </Typography.Text>
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
              maximized
                ? i18nText('frontstage', 'auto.restore_window')
                : i18nText('frontstage', 'auto.maximize_window')
            }
            disabled={mobile}
            icon={maximized ? <CompressOutlined /> : <FullscreenOutlined />}
            onClick={() => setMaximized((value) => !value)}
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

        {activeSection === 'code' ? null : (
          <aside className="frontstage-jsx-studio__resource-panel">
            <JsxStudioResourcePanel
              block={block}
              pageBlocks={pageBlocks}
              callableInterfaces={callableInterfaces.data}
              callableInterfacesError={callableInterfaces.error}
              callableInterfacesLoading={callableInterfaces.loading}
              onInsertCode={insertCode}
              onSaveBlock={onSaveBlock}
              projection={projection}
              runPanel={resolvedRunPanel}
              section={activeSection}
            />
          </aside>
        )}

        <main className="frontstage-jsx-studio__editor-panel">
          {permissionDenied ? <PermissionDeniedState /> : null}
          {error && !permissionDenied ? (
            <Alert
              type="error"
              showIcon
              message={i18nText('frontstage', 'auto.code_load_or_save_failed')}
            />
          ) : null}
          <div className="frontstage-jsx-studio__context-actions">
            <Button size="small" onClick={reinjectContext}>
              {i18nText('frontstage', 'auto.generated_context')}
            </Button>
          </div>
          <div className="frontstage-jsx-studio__monaco">
            <Editor
              height="100%"
              language="typescript"
              path={`file:///frontstage/${pageId}/${tabId ?? 'tab'}/${block.id}.tsx`}
              value={draft}
              beforeMount={configureMonaco}
              onMount={(editor) => {
                editorRef.current = editor;
              }}
              onChange={(value) => setDraft(value ?? '')}
              options={{
                automaticLayout: true,
                editContext: false,
                fontSize: 13,
                lineNumbersMinChars: 3,
                minimap: { enabled: false },
                padding: { top: 12, bottom: 12 },
                readOnly: loading || saving || permissionDenied,
                scrollBeyondLastLine: false,
                tabSize: 2,
                wordWrap: 'on'
              }}
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
