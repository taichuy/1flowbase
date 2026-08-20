import type { OnMount } from '@monaco-editor/react';
import {
  createJsBlockDiagnostics,
  diagnoseLegacyBlockModuleSource,
  validateNativeTrustedBlockSource
} from '@1flowbase/page-runtime';
import { Alert, App, Empty, Form, Input, Select } from 'antd';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { BlockSourceStudio } from '../../../../shared/code-block/BlockSourceStudio';
import { diagnoseUnsupportedTailwindUtilities } from '../../../../shared/code-block/tailwind-utility-diagnostics';
import { useFrontstageBlockCatalog } from '../../../frontstage/hooks/use-frontstage-block-catalog';
import { createFrontstageJsxEditorProjection } from '../../../frontstage/lib/jsx-studio/editor-projection';
import { injectFrontstageContextComment } from '../../../frontstage/lib/jsx-studio/context-injection';
import {
  applyFrontstageJsxInsertionPlan,
  planFrontstageJsxInsertion,
  type FrontstageJsxInsertion
} from '../../../frontstage/lib/jsx-studio/source-insertion';
import type { FrontstageBlockInstance } from '../../../frontstage/lib/page-document';
import {
  JsxStudioRunPanel,
  type JsxStudioRunBlockContextInput
} from '../../../frontstage/components/jsx-studio/JsxStudioRunPanel';
import { createFrontstageUnavailableBlockContext } from '../../../frontstage/lib/native-trusted-block-react-adapter';
import { JsxStudioResourcePanel } from '../../../frontstage/components/jsx-studio/JsxStudioResourcePanel';
import type {
  SettingsUiOfficialTemplate,
  SettingsUiTemplateInput
} from '../../api/ui-management';

export type UiCodeTemplateStudioMode = 'create' | 'copy' | 'edit' | 'view';

export function UiCodeTemplateStudio({
  initialValue,
  mode,
  officialTemplates,
  open,
  saving,
  workspaceId,
  onClose,
  onSave
}: {
  initialValue: SettingsUiTemplateInput;
  mode: UiCodeTemplateStudioMode;
  officialTemplates: SettingsUiOfficialTemplate[];
  open: boolean;
  saving: boolean;
  workspaceId: string | null;
  onClose: () => void;
  onSave: (value: SettingsUiTemplateInput) => void;
}) {
  const { t } = useTranslation('settingsUiManagement');
  const { message } = App.useApp();
  const [draft, setDraft] = useState(initialValue);
  const [previewRequest, setPreviewRequest] = useState<{
    revision: string;
    source: string;
  } | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const previewSequenceRef = useRef(0);
  const catalog = useFrontstageBlockCatalog({ workspaceId });
  const catalogEntry =
    catalog.items.find(
      (entry) =>
        entry.providerCode === draft.provider_code &&
        entry.contributionCode === draft.contribution_code
    ) ?? null;
  const projection = useMemo(
    () => createFrontstageJsxEditorProjection({ catalogEntry }),
    [catalogEntry]
  );
  const readOnly = mode === 'view';
  const contributionIdentity = templateIdentity(
    draft.provider_code,
    draft.contribution_code
  );
  const dirty =
    mode === 'copy' ||
    (mode !== 'view' && JSON.stringify(draft) !== JSON.stringify(initialValue));
  const authoringBlock = useMemo(
    () => createAuthoringBlock(draft, catalogEntry),
    [catalogEntry, draft]
  );
  const diagnostics = useMemo(() => {
    if (draft.source.trim().length === 0) return [];
    const legacy = diagnoseLegacyBlockModuleSource(draft.source);
    const values = legacy
      ? [legacy]
      : (() => {
          const validation = validateNativeTrustedBlockSource(draft.source, {
            allowedImportSources: projection.allowedImportSources
          });
          return validation.ok
            ? diagnoseUnsupportedTailwindUtilities(draft.source)
            : validation.errors;
        })();
    return createJsBlockDiagnostics(
      {
        pageId: 'ui-code-template',
        tabId: 'authoring',
        blockId: contributionIdentity || 'new-template'
      },
      values
    );
  }, [contributionIdentity, draft.source, projection.allowedImportSources]);

  useEffect(() => {
    if (!open) return;
    setDraft(initialValue);
    setPreviewRequest(null);
    previewSequenceRef.current = 0;
  }, [initialValue, open]);

  const createPreviewBlockContext = useCallback(
    ({ plan }: JsxStudioRunBlockContextInput) => {
      const unavailable = createFrontstageUnavailableBlockContext(plan);
      return {
        ...unavailable,
        workspace: { id: workspaceId ?? 'workspace' }
      };
    },
    [workspaceId]
  );

  const insertCode = (insertion: FrontstageJsxInsertion) => {
    const editor = editorRef.current;
    const selection = editor?.getSelection();
    const model = editor?.getModel();
    if (editor && selection && model) {
      const insertionPlan = planFrontstageJsxInsertion({
        source: model.getValue(),
        selection: {
          start: model.getOffsetAt(selection.getStartPosition()),
          end: model.getOffsetAt(selection.getEndPosition())
        },
        insertion
      });
      editor.pushUndoStop();
      editor.executeEdits(
        'ui-code-template-studio',
        insertionPlan.edits.map((edit) => {
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
    const insertionPlan = planFrontstageJsxInsertion({
      source: draft.source,
      selection: { start: draft.source.length, end: draft.source.length },
      insertion
    });
    setDraft((current) => ({
      ...current,
      source: applyFrontstageJsxInsertionPlan(current.source, insertionPlan)
    }));
  };

  const submit = () => {
    if (!draft.provider_code || !draft.contribution_code) {
      void message.error(t('select_contribution_required'));
      return;
    }
    if (!draft.name.trim()) {
      void message.error(t('template_name_required'));
      return;
    }
    if (!draft.source.trim()) {
      void message.error(t('template_source_required'));
      return;
    }
    onSave({ ...draft, name: draft.name.trim() });
  };

  const identityForm = (
    <div className="frontstage-jsx-studio__resource-scroll">
      <section className="frontstage-jsx-studio__resource-section">
        <Form layout="vertical">
          <Form.Item label={t('template_contribution')} required>
            <Select
              aria-label={t('template_contribution')}
              disabled={mode !== 'create'}
              value={contributionIdentity || undefined}
              options={officialTemplates.map((template) => ({
                value: templateIdentity(
                  template.provider_code,
                  template.contribution_code
                ),
                label: `${template.title} · ${template.provider_code}/${template.contribution_code}`
              }))}
              onChange={(identity) => {
                const template = officialTemplates.find(
                  (candidate) =>
                    templateIdentity(
                      candidate.provider_code,
                      candidate.contribution_code
                    ) === identity
                );
                if (!template) return;
                setDraft((current) => ({
                  ...current,
                  provider_code: template.provider_code,
                  contribution_code: template.contribution_code,
                  source: template.source,
                  language: template.language
                }));
              }}
            />
          </Form.Item>
          <Form.Item label={t('name')} required>
            <Input
              aria-label={t('name')}
              disabled={readOnly}
              value={draft.name}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  name: event.target.value
                }))
              }
            />
          </Form.Item>
          <Form.Item label={t('language')} required>
            <Select
              aria-label={t('language')}
              disabled={readOnly}
              value={draft.language}
              options={[
                { value: 'tsx', label: 'TSX' },
                { value: 'jsx', label: 'JSX' }
              ]}
              onChange={(language) =>
                setDraft((current) => ({ ...current, language }))
              }
            />
          </Form.Item>
        </Form>
      </section>
    </div>
  );

  return (
    <BlockSourceStudio
      contextComment={projection.contextComment}
      dirty={dirty}
      editorDiagnostics={diagnostics}
      extraLibs={projection.monacoExtraLibs}
      initialSection="configuration"
      loading={false}
      open={open}
      owner="settings:ui-management:code-templates"
      path={`file:///settings/ui-code-templates/${encodeURIComponent(contributionIdentity || 'new')}.${draft.language}`}
      readOnly={readOnly}
      saving={saving}
      sections={
        workspaceId
          ? ['code', 'interfaces', 'components', 'configuration', 'run']
          : ['code', 'configuration', 'run']
      }
      source={draft.source}
      testId="ui-code-template-studio"
      windowId="ui-code-template-studio"
      onChange={(source) => setDraft((current) => ({ ...current, source }))}
      onClose={onClose}
      onEditorMount={(editor) => {
        editorRef.current = editor;
      }}
      onInjectContext={injectFrontstageContextComment}
      onReset={() => setDraft(initialValue)}
      onRun={(source) => {
        if (diagnoseLegacyBlockModuleSource(source)) return;
        previewSequenceRef.current += 1;
        setPreviewRequest({
          revision: `run:${previewSequenceRef.current}`,
          source
        });
      }}
      onSave={submit}
      renderResource={(section) =>
        section === 'configuration' ? (
          identityForm
        ) : section === 'run' ? (
          previewRequest ? (
            <div className="frontstage-jsx-studio__resource-scroll">
              <Alert
                showIcon
                type="info"
                title={t('template_preview_limited')}
              />
              <JsxStudioRunPanel
                block={authoringBlock}
                code={previewRequest.source}
                createBlockContext={createPreviewBlockContext}
                revision={previewRequest.revision}
              />
            </div>
          ) : (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={t('template_preview_idle')}
            />
          )
        ) : !workspaceId ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={t('template_resources_unavailable')}
          />
        ) : (
          <JsxStudioResourcePanel
            block={authoringBlock}
            codeSource={draft.source}
            pageBlocks={[authoringBlock]}
            projection={projection}
            section={section}
            workspaceId={workspaceId}
            onInsertCode={insertCode}
            onSaveBlock={async () => false}
          />
        )
      }
    />
  );
}

function templateIdentity(providerCode: string, contributionCode: string) {
  return providerCode && contributionCode
    ? `${providerCode}:${contributionCode}`
    : '';
}

function createAuthoringBlock(
  draft: SettingsUiTemplateInput,
  catalogEntry:
    | ReturnType<typeof useFrontstageBlockCatalog>['items'][number]
    | null
): FrontstageBlockInstance {
  return {
    id: `ui-code-template:${templateIdentity(draft.provider_code, draft.contribution_code) || 'new'}`,
    rendererVersion: 'v1',
    sourceId: 'ui-code-template',
    codeRef: 'ui-code-template',
    sourceCodeRef: 'ui-code-template',
    catalog: {
      providerCode: draft.provider_code,
      installationId: catalogEntry?.installationId ?? 'system'
    },
    contribution: {
      pluginId: catalogEntry?.pluginId ?? draft.provider_code,
      pluginVersion: catalogEntry?.pluginVersion ?? 'registered',
      code: draft.contribution_code
    },
    props: {},
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: 'native_trusted_block',
      entry: catalogEntry?.entry ?? 'index.js',
      hint: 'native_trusted_block'
    }
  };
}
