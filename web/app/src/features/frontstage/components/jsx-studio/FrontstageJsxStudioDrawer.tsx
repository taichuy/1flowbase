import {
  ApiOutlined,
  AppstoreOutlined,
  CodeOutlined,
  DatabaseOutlined,
  PlayCircleOutlined,
  SettingOutlined
} from '@ant-design/icons';
import Editor, { type BeforeMount, type OnMount } from '@monaco-editor/react';
import type { BlockRuntimeDiagnostic } from '@1flowbase/page-protocol';
import {
  createJsBlockDiagnostics,
  validateJsBlockSource
} from '@1flowbase/page-runtime';
import { Alert, Button, Space, Tooltip, Typography } from 'antd';
import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
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

  return (
    <ResizableDrawer
      open={open}
      title={i18nText('frontstage', 'auto.jsx_studio')}
      defaultWidth={960}
      minWidth={680}
      maxWidth={1440}
      resizeLabel={i18nText('frontstage', 'auto.resize_jsx_studio')}
      rootClassName="frontstage-jsx-studio"
      bodyClassName="frontstage-jsx-studio__drawer-body"
      onClose={onClose}
      extra={
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
        </Space>
      }
    >
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
    </ResizableDrawer>
  );
}
