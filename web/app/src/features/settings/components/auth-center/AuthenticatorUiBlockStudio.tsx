import type { OnMount } from '@monaco-editor/react';
import { Descriptions } from 'antd';
import { useEffect, useMemo, useRef, useState } from 'react';

import { BlockSourceStudio } from '../../../../shared/code-block/BlockSourceStudio';
import { i18nText } from '../../../../shared/i18n/text';
import {
  createPublicAuthInputs,
  createPublicAuthPreviewCapabilityHandlers,
  PUBLIC_AUTH_RUNTIME_LIMITS
} from '../../../auth/components/public-auth-block-host';
import { JsBlockTrialPanel } from '../../../frontstage/components/JsBlockTrialPanel';
import {
  JsxStudioResourcePanel,
  type JsxStudioContextVariable
} from '../../../frontstage/components/jsx-studio/JsxStudioResourcePanel';
import { useFrontstageBlockCatalog } from '../../../frontstage/hooks/use-frontstage-block-catalog';
import { createFrontstageJsxEditorProjection } from '../../../frontstage/lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../../frontstage/lib/jsx-studio/context-injection';
import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion,
  type FrontstageJsxInsertion
} from '../../../frontstage/lib/jsx-studio/source-insertion';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../../frontstage/lib/block-catalog';
import type { FrontstageBlockInstance } from '../../../frontstage/lib/page-document';

export interface AuthenticatorUiBlockStudioProps {
  authenticatorId: string;
  authenticatorTitle: string;
  authType: string;
  description: string | null;
  enabled: boolean;
  errorMessage: string | null;
  contextVariables?: readonly JsxStudioContextVariable[];
  interfacePathPrefixes: readonly string[];
  publicVariables: Record<string, unknown> | null;
  open: boolean;
  readOnly: boolean;
  saving: boolean;
  selfRegistrationEnabled: boolean;
  source: string;
  workspaceId: string;
  onClose: () => void;
  onSave: (source: string) => Promise<void>;
}

const AUTH_CONTEXT_COMMENT = [
  '/**',
  ' * @1flowbase-context',
  ' * inputs: authenticator_id, public_variables, auth_event',
  ' * interfaces: ctx.api',
  ' * outputs: 无',
  ' */'
].join('\n');
export function AuthenticatorUiBlockStudio({
  authenticatorId,
  authenticatorTitle,
  authType,
  contextVariables,
  description,
  enabled,
  errorMessage,
  interfacePathPrefixes,
  publicVariables,
  onClose,
  onSave,
  open,
  readOnly,
  saving,
  selfRegistrationEnabled,
  source,
  workspaceId
}: AuthenticatorUiBlockStudioProps) {
  const blockCatalog = useFrontstageBlockCatalog({ workspaceId });
  const authoringCatalogEntry = blockCatalog.items.find(
    (entry) =>
      entry.providerCode === '1flowbase' &&
      entry.contributionCode === 'frontstage.js-ui-block'
  ) ?? null;
  const editorProjection = useMemo(
    () => ({
      ...createFrontstageJsxEditorProjection({
        catalogEntry: authoringCatalogEntry
      }),
      contextComment: AUTH_CONTEXT_COMMENT
    }),
    [authoringCatalogEntry]
  );
  const [draft, setDraft] = useState(source);
  const [previewRequest, setPreviewRequest] = useState<{
    revision: string;
    source: string;
  } | null>(null);
  const previewSequenceRef = useRef(0);
  const [authoringBlock, setAuthoringBlock] = useState<FrontstageBlockInstance>(
    () => createAuthoringBlock(authenticatorId, authoringCatalogEntry)
  );
  const previewCapabilities = useMemo(
    () => createPublicAuthPreviewCapabilityHandlers(),
    []
  );
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  useEffect(() => {
    if (!open) return;
    setDraft(source);
    setAuthoringBlock(
      createAuthoringBlock(authenticatorId, authoringCatalogEntry)
    );
  }, [authenticatorId, authoringCatalogEntry, open, source]);

  useEffect(() => {
    if (!open) return;
    previewSequenceRef.current = 0;
    setPreviewRequest(null);
  }, [authenticatorId, open]);

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
        'auth-center-jsx-studio',
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
    const nextSource = `${draft}${separator}`;
    const plan = planFrontstageJsxInsertion({
      source: nextSource,
      selection: { start: nextSource.length, end: nextSource.length },
      insertion
    });
    setDraft(`${applyFrontstageJsxInsertionPlan(nextSource, plan)}\n`);
  };

  return (
    <BlockSourceStudio
      contextComment={AUTH_CONTEXT_COMMENT}
      dirty={draft !== source}
      errorMessage={errorMessage}
      extraLibs={editorProjection.monacoExtraLibs}
      initialSection="code"
      loading={false}
      open={open}
      owner="settings:auth-center"
      path={`file:///auth-center/${authenticatorId}/public-ui-block.tsx`}
      readOnly={readOnly}
      saving={saving}
      source={draft}
      testId={`auth-center-jsx-studio-${authenticatorId}`}
      windowId={`auth-center-jsx-studio:${authenticatorId}`}
      onChange={setDraft}
      onClose={onClose}
      onEditorMount={(editor) => {
        editorRef.current = editor;
      }}
      onInjectContext={injectFrontstageContextComment}
      onReset={() => setDraft(source)}
      onRun={(runSource) => {
        previewSequenceRef.current += 1;
        setPreviewRequest({
          revision: `run:${previewSequenceRef.current}`,
          source: runSource
        });
      }}
      onSave={() => void onSave(draft)}
      renderResource={(section) => (
        <JsxStudioResourcePanel
          block={authoringBlock}
          codeSource={draft}
          contextVariables={contextVariables ?? null}
          interfacePathPrefixes={interfacePathPrefixes}
          pageBlocks={[authoringBlock]}
          projection={editorProjection}
          runPanel={
            publicVariables && authoringCatalogEntry && previewRequest ? (
              <JsBlockTrialPanel
                key={authenticatorId}
                block={authoringBlock}
                catalogEntry={authoringCatalogEntry}
                code={previewRequest.source}
                contextSnapshot={{}}
                createRunInputs={(event) =>
                  createPublicAuthInputs(
                    authenticatorId,
                    publicVariables,
                    event
                  )
                }
                handlers={previewCapabilities}
                limits={PUBLIC_AUTH_RUNTIME_LIMITS}
                onPrepareDraftRun={previewCapabilities.prepareDraftRun}
                onRevokeDraftRun={previewCapabilities.revokeDraftRun}
                presentation={{
                  mode: 'direct-preview',
                  revision: previewRequest.revision
                }}
              />
            ) : undefined
          }
          section={section}
          workspaceId={workspaceId}
          configurationPanel={(
            <div className="frontstage-jsx-studio__resource-scroll">
              <Descriptions
                column={1}
                size="small"
                items={[
                  {
                    key: 'title',
                    label: i18nText('settings', 'auto.name'),
                    children: authenticatorTitle
                  },
                  {
                    key: 'type',
                    label: i18nText(
                      'settings',
                      'auto.auth_center_auth_type'
                    ),
                    children: authType
                  },
                  {
                    key: 'description',
                    label: i18nText('settings', 'auto.description'),
                    children: description || '-'
                  },
                  {
                    key: 'enabled',
                    label: i18nText('settings', 'auto.enabled'),
                    children: i18nText(
                      'settings',
                      enabled ? 'auto.yes' : 'auto.no'
                    )
                  },
                  {
                    key: 'registration',
                    label: i18nText(
                      'settings',
                      'auto.auth_center_self_registration'
                    ),
                    children: i18nText(
                      'settings',
                      selfRegistrationEnabled ? 'auto.yes' : 'auto.no'
                    )
                  }
                ]}
              />
            </div>
          )}
          onInsertCode={insertCode}
          onSaveBlock={async (block) => {
            setAuthoringBlock(block);
            return true;
          }}
        />
      )}
    />
  );
}

function createAuthoringBlock(
  authenticatorId: string,
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null
): FrontstageBlockInstance {
  return {
    id: `public-auth:${authenticatorId}`,
    rendererVersion: 'v1',
    sourceId: `public-auth:${authenticatorId}`,
    codeRef: `public-auth:${authenticatorId}`,
    sourceCodeRef: `public-auth:${authenticatorId}`,
    catalog: {
      providerCode: catalogEntry?.providerCode ?? '1flowbase',
      installationId: catalogEntry?.installationId ?? 'builtin-installation'
    },
    contribution: {
      pluginId: catalogEntry?.pluginId ?? 'builtin-frontstage',
      pluginVersion: catalogEntry?.pluginVersion ?? '1.0.0',
      code: catalogEntry?.contributionCode ?? 'frontstage.js-ui-block'
    },
    props: {},
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: catalogEntry?.runtimeKind ?? 'iframe',
      entry: catalogEntry?.entry ?? 'index.js',
      hint: catalogEntry?.runtimeKind ?? 'iframe'
    }
  };
}
