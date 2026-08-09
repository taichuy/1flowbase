import { useEffect, useState } from 'react';

import {
  Alert,
  Button,
  Drawer,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';

import type {
  CreateSettingsDataModelInput,
  SettingsCompatibleDataModelTemplate,
  SettingsDataModel,
  SettingsDataSource,
  UpdateSettingsDataModelInput
} from '../../api/data-models';
import { DataModelFieldLabel } from './DataModelHelpTooltip';
import {
  dataModelCodeHelp,
  dataModelStatusHelp,
  dataModelTitleHelp
} from './data-model-help-text';
import { i18nText } from '../../../../shared/i18n/text';
import {
  dataModelTemplateIdentity,
  dataModelTemplatePresentation
} from '../../lib/data-model-template-presentation';

interface DataModelFormValues {
  code: string;
  title: string;
  description: string;
}

function isApiOpen(status: SettingsDataModel['status'] | undefined) {
  return status === 'published';
}

function apiOpenStatus(apiOpen: boolean): SettingsDataModel['status'] {
  return apiOpen ? 'published' : 'draft';
}

export function DataModelFormDrawer({
  open,
  mode,
  model,
  source,
  compatibleTemplates,
  templatesLoading,
  templatesError,
  saving,
  onClose,
  onCreate,
  onUpdate
}: {
  open: boolean;
  mode: 'create' | 'edit';
  model: SettingsDataModel | null;
  source: SettingsDataSource | null;
  compatibleTemplates: SettingsCompatibleDataModelTemplate[];
  templatesLoading: boolean;
  templatesError: string | null;
  saving: boolean;
  onClose: () => void;
  onCreate: (input: CreateSettingsDataModelInput) => void;
  onUpdate: (
    model: SettingsDataModel,
    input: UpdateSettingsDataModelInput
  ) => void;
}) {
  const [form] = Form.useForm<DataModelFormValues>();
  const [apiOpen, setApiOpen] = useState(true);
  const [selectedTemplate, setSelectedTemplate] =
    useState<SettingsCompatibleDataModelTemplate | null>(null);
  useEffect(() => {
    if (!open) {
      return;
    }

    if (mode === 'edit' && model) {
      setApiOpen(isApiOpen(model.status));
      form.setFieldsValue({
        code: model.code,
        title: model.title,
        description: model.description ?? ''
      });
      return;
    }

    setApiOpen(isApiOpen(source?.default_data_model_status ?? 'published'));
    form.setFieldsValue({
      code: '',
      title: '',
      description: ''
    });
  }, [form, mode, model, open, source]);

  useEffect(() => {
    if (!open || mode !== 'create') {
      setSelectedTemplate(null);
      return;
    }

    setSelectedTemplate(
      compatibleTemplates.find(
        (template) =>
          template.template_provider === 'core' &&
          template.template_code === 'general' &&
          template.template_version === 'v1'
      ) ?? null
    );
  }, [compatibleTemplates, mode, open]);

  const handleSubmit = async () => {
    const values = await form.validateFields();

    if (mode === 'edit' && model) {
      onUpdate(model, {
        title: values.title,
        description: values.description.trim() || null,
        status: apiOpenStatus(apiOpen)
      });
      onClose();
      return;
    }

    if (!selectedTemplate) {
      return;
    }

    onCreate({
      scope_kind: 'workspace',
      template_provider: selectedTemplate.template_provider,
      template_code: selectedTemplate.template_code,
      template_version: selectedTemplate.template_version,
      code: values.code,
      title: values.title,
      description: values.description.trim() || null,
      status: apiOpenStatus(apiOpen)
    });
    onClose();
  };

  return (
    <Drawer
      title={
        mode === 'create'
          ? i18nText('settings', 'auto.new_data_model')
          : i18nText('settings', 'auto.edit_data_model')
      }
      open={open}
      size={520}
      onClose={onClose}
      extra={
        <Button
          type="primary"
          aria-label={
            mode === 'create'
              ? i18nText('settings', 'auto.create')
              : i18nText('settings', 'auto.save')
          }
          loading={saving}
          disabled={
            mode === 'create' &&
            (templatesLoading || Boolean(templatesError) || !selectedTemplate)
          }
          onClick={handleSubmit}
        >
          {mode === 'create'
            ? i18nText('settings', 'auto.create')
            : i18nText('settings', 'auto.save')}
        </Button>
      }
    >
      <Form form={form} layout="vertical">
        {mode === 'create' ? (
          <>
            {templatesError ? (
              <Alert type="error" showIcon title={templatesError} />
            ) : null}
            {!templatesLoading &&
            !templatesError &&
            compatibleTemplates.length === 0 ? (
              <Alert
                type="warning"
                showIcon
                title={i18nText(
                  'settings',
                  'auto.no_compatible_data_model_template'
                )}
              />
            ) : null}
            <Form.Item
              label={i18nText('settings', 'auto.data_model_template')}
              required
            >
              <Select
                aria-label={i18nText('settings', 'auto.data_model_template')}
                loading={templatesLoading}
                disabled={
                  templatesLoading ||
                  Boolean(templatesError) ||
                  compatibleTemplates.length === 0
                }
                value={
                  selectedTemplate
                    ? dataModelTemplateIdentity(selectedTemplate)
                    : undefined
                }
                placeholder={i18nText(
                  'settings',
                  'auto.select_data_model_template'
                )}
                options={compatibleTemplates.map((template) => ({
                  value: dataModelTemplateIdentity(template),
                  label: (
                    <Space orientation="vertical" size={0}>
                      <Typography.Text strong>
                        {dataModelTemplatePresentation(template).title}
                      </Typography.Text>
                      <Typography.Text type="secondary">
                        {dataModelTemplatePresentation(template).description}
                      </Typography.Text>
                      <Typography.Text type="secondary">
                        {dataModelTemplateIdentity(template)}
                      </Typography.Text>
                    </Space>
                  )
                }))}
                onChange={(identity) =>
                  setSelectedTemplate(
                    compatibleTemplates.find(
                      (template) =>
                        dataModelTemplateIdentity(template) === identity
                    ) ?? null
                  )
                }
              />
            </Form.Item>
          </>
        ) : null}
        <Form.Item
          name="title"
          label={
            <DataModelFieldLabel
              label={i18nText('settings', 'auto.title')}
              title={dataModelTitleHelp}
            />
          }
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.enter_title')
            }
          ]}
        >
          <Input aria-label={i18nText('settings', 'auto.title')} />
        </Form.Item>
        <Form.Item
          name="description"
          label={i18nText('settings', 'auto.description')}
        >
          <Input.TextArea
            aria-label={i18nText('settings', 'auto.description')}
            autoSize={{ minRows: 3, maxRows: 6 }}
          />
        </Form.Item>
        <Form.Item
          name="code"
          label={<DataModelFieldLabel label="Code" title={dataModelCodeHelp} />}
          rules={[
            {
              required: true,
              message: i18nText('settings', 'auto.enter_data_model_code')
            }
          ]}
        >
          <Input aria-label="Code" disabled={mode === 'edit'} />
        </Form.Item>
        <Form.Item
          label={
            <DataModelFieldLabel
              label={i18nText('settings', 'auto.open_api')}
              title={dataModelStatusHelp}
            />
          }
        >
          <Switch
            aria-label={i18nText('settings', 'auto.open_api')}
            checked={apiOpen}
            checkedChildren={i18nText('settings', 'auto.open')}
            unCheckedChildren={i18nText('settings', 'auto.closed')}
            disabled={saving}
            onChange={setApiOpen}
          />
        </Form.Item>
      </Form>
    </Drawer>
  );
}
