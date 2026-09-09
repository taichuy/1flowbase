import {
  Alert,
  App,
  Button,
  Empty,
  Form,
  Input,
  Modal,
  Radio,
  Space,
  Spin,
  Typography
} from 'antd';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';

import { useAuthStore } from '../../../../state/auth-store';
import {
  createSettingsUiTemplate,
  settingsUiTemplatesQueryKey,
  type SettingsUiTemplateInput
} from '../../../settings/api/ui-management';
import { isForbiddenResponseError } from '../../lib/api-errors';
import { i18nText } from '../../../../shared/i18n/text';
import { useFrontstageUiTemplates } from '../../hooks/use-frontstage-ui-templates';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';

export function JsxStudioTemplatesPanel({
  catalogEntry,
  source,
  blockTitle,
  onReplaceCode,
  readOnly,
  workspaceId
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  source: string;
  blockTitle: string;
  onReplaceCode: (source: string) => void;
  readOnly: boolean;
  workspaceId: string;
}) {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const canManageTemplates = useAuthStore(
    (state) =>
      state.actor?.effective_display_role === 'root' ||
      (state.me?.permissions.includes(
        'settings_feature.access.system.ui-management'
      ) ??
        false)
  );
  const [form] = Form.useForm<{ name: string }>();
  const [templateDraft, setTemplateDraft] =
    useState<SettingsUiTemplateInput | null>(null);
  const canSaveTemplate =
    !readOnly &&
    canManageTemplates &&
    !!csrfToken &&
    !!catalogEntry &&
    source.trim().length > 0;
  const saveTemplate = useMutation({
    mutationFn: (input: SettingsUiTemplateInput) =>
      createSettingsUiTemplate(input, csrfToken!),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: settingsUiTemplatesQueryKey
      });
      setTemplateDraft(null);
      void message.success(i18nText('frontstage', 'auto.template_draft_saved'));
    }
  });
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>();
  const templatesQuery = useFrontstageUiTemplates(workspaceId);
  const templates = useMemo(
    () =>
      (templatesQuery.data ?? []).filter(
        (template) =>
          template.provider_code === catalogEntry?.providerCode &&
          template.contribution_code === catalogEntry?.contributionCode
      ),
    [
      catalogEntry?.contributionCode,
      catalogEntry?.providerCode,
      templatesQuery.data
    ]
  );
  const selectedTemplate = templates.find(
    (template) =>
      templateIdentity(template.template_id, template.version) ===
      selectedTemplateId
  );

  const replaceCode = () => {
    if (!selectedTemplate) return;
    Modal.confirm({
      title: i18nText('frontstage', 'auto.replace_code_with_template'),
      content: i18nText(
        'frontstage',
        'auto.replace_code_with_template_confirm'
      ),
      okText: i18nText('frontstage', 'auto.replace'),
      cancelText: i18nText('frontstage', 'auto.cancel'),
      onOk: () => onReplaceCode(selectedTemplate.source)
    });
  };

  return (
    <div className="frontstage-jsx-studio__resource-scroll">
      <section className="frontstage-jsx-studio__resource-section">
        <Typography.Title level={5}>
          {i18nText('frontstage', 'auto.code_template')}
        </Typography.Title>
        <Typography.Paragraph type="secondary">
          {i18nText('frontstage', 'auto.code_template_description')}
        </Typography.Paragraph>
        <Button
          block
          disabled={!canSaveTemplate}
          onClick={() => {
            if (!catalogEntry) return;
            saveTemplate.reset();
            form.resetFields();
            form.setFieldsValue({ name: blockTitle });
            setTemplateDraft({
              name: blockTitle,
              source,
              language: 'tsx',
              provider_code: catalogEntry.providerCode,
              contribution_code: catalogEntry.contributionCode
            });
          }}
        >
          {i18nText('frontstage', 'auto.save_as_page_template')}
        </Button>
        <Spin spinning={templatesQuery.isLoading}>
          {templates.length > 0 ? (
            <Radio.Group
              disabled={readOnly}
              value={selectedTemplateId}
              onChange={(event) => setSelectedTemplateId(event.target.value)}
            >
              <Space orientation="vertical">
                {templates.map((template) => (
                  <Radio
                    key={templateIdentity(
                      template.template_id,
                      template.version
                    )}
                    value={templateIdentity(
                      template.template_id,
                      template.version
                    )}
                  >
                    {`${template.name} · ${template.version}${template.is_default ? ` · ${i18nText('frontstage', 'auto.default')}` : ''}`}
                  </Radio>
                ))}
              </Space>
            </Radio.Group>
          ) : (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText(
                'frontstage',
                'auto.no_available_code_templates'
              )}
            />
          )}
        </Spin>
        <Button
          block
          danger
          disabled={readOnly || !selectedTemplate}
          onClick={replaceCode}
        >
          {i18nText('frontstage', 'auto.replace_current_code')}
        </Button>
      </section>
      <Modal
        getContainer={false}
        open={templateDraft !== null}
        title={i18nText('frontstage', 'auto.save_as_page_template')}
        okText={i18nText('frontstage', 'auto.save_template_draft')}
        cancelText={i18nText('frontstage', 'auto.cancel')}
        confirmLoading={saveTemplate.isPending}
        okButtonProps={{ disabled: !canSaveTemplate }}
        cancelButtonProps={{ disabled: saveTemplate.isPending }}
        closable={!saveTemplate.isPending}
        maskClosable={!saveTemplate.isPending}
        keyboard={!saveTemplate.isPending}
        onCancel={() => setTemplateDraft(null)}
        onOk={() => form.submit()}
      >
        <Typography.Paragraph type="secondary">
          {i18nText('frontstage', 'auto.save_template_draft_description')}
        </Typography.Paragraph>
        <Form
          form={form}
          layout="vertical"
          disabled={saveTemplate.isPending}
          onFinish={({ name }) => {
            if (!templateDraft || !canSaveTemplate || saveTemplate.isPending)
              return;
            saveTemplate.mutate({ ...templateDraft, name: name.trim() });
          }}
        >
          <Form.Item
            name="name"
            label={i18nText('frontstage', 'auto.template_name')}
            rules={[
              {
                required: true,
                whitespace: true,
                message: i18nText('frontstage', 'auto.template_name_required')
              }
            ]}
          >
            <Input autoFocus />
          </Form.Item>
        </Form>
        {saveTemplate.isError && (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'frontstage',
              isForbiddenResponseError(saveTemplate.error)
                ? 'auto.template_create_forbidden'
                : 'auto.template_save_failed'
            )}
          />
        )}
      </Modal>
    </div>
  );
}

function templateIdentity(templateId: string | null, version: string) {
  return `${templateId ?? 'official'}:${version}`;
}
