import { Button, Empty, Modal, Radio, Space, Spin, Typography } from 'antd';
import { useMemo, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { useFrontstageUiTemplates } from '../../hooks/use-frontstage-ui-templates';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';

export function JsxStudioTemplatesPanel({
  catalogEntry,
  onReplaceCode,
  readOnly,
  workspaceId
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  onReplaceCode: (source: string) => void;
  readOnly: boolean;
  workspaceId: string;
}) {
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
    </div>
  );
}

function templateIdentity(templateId: string | null, version: string) {
  return `${templateId ?? 'official'}:${version}`;
}
