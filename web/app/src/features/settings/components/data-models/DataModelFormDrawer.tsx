import { useEffect, useMemo, useState } from 'react';

import { Select, Space, Table, Tag, Typography } from 'antd';
import type { ColumnsType } from 'antd/es/table';

import type {
  CreateSettingsDataModelInput,
  SettingsCompatibleDataModelTemplate,
  SettingsDataModel,
  SettingsDataSource,
  UpdateSettingsDataModelInput
} from '../../api/data-models';
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
import { SchemaFormDrawer } from '../../../../shared/schema-ui/v1/form-drawer/SchemaFormDrawer';
import type { SchemaFormValues } from '../../../../shared/schema-ui/v1/form-drawer/SchemaFormDrawer';
import type { PluginFormSchema } from '../../../../shared/schema-ui/v1/contracts/plugin-form-schema';

function isApiOpen(status: SettingsDataModel['status'] | undefined) {
  return status === 'published';
}

function apiOpenStatus(apiOpen: boolean): SettingsDataModel['status'] {
  return apiOpen ? 'published' : 'draft';
}

const systemFieldColumns: ColumnsType<
  SettingsCompatibleDataModelTemplate['system_fields'][number]
> = [
  {
    title: 'Code',
    dataIndex: 'code',
    key: 'code',
    render: (value: string) => (
      <code className="data-model-panel__code-badge">{value}</code>
    )
  },
  {
    title: i18nText('settings', 'auto.kind'),
    dataIndex: 'field_kind',
    key: 'field_kind',
    render: (value: string) => <Tag>{value}</Tag>
  },
  {
    title: i18nText('settings', 'auto.required'),
    dataIndex: 'required',
    key: 'required',
    width: 80,
    render: (value: boolean) =>
      value ? (
        <Tag color="error">{i18nText('settings', 'auto.required')}</Tag>
      ) : (
        <Typography.Text type="secondary">-</Typography.Text>
      )
  }
];

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
  const [selectedTemplate, setSelectedTemplate] =
    useState<SettingsCompatibleDataModelTemplate | null>(null);

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

  const schema = useMemo<PluginFormSchema>(
    () => ({
      schema_version: '1.0.0',
      fields: [
        {
          key: 'title',
          label: i18nText('settings', 'auto.title'),
          description: dataModelTitleHelp,
          type: 'string',
          required: true
        },
        {
          key: 'code',
          label: 'Code',
          description: dataModelCodeHelp,
          type: 'string',
          required: true,
          read_only: mode === 'edit'
        },
        {
          key: 'description',
          label: i18nText('settings', 'auto.description'),
          type: 'string',
          control: 'textarea'
        },
        {
          key: 'api_open',
          label: i18nText('settings', 'auto.open_api'),
          description: dataModelStatusHelp,
          type: 'boolean'
        }
      ]
    }),
    [mode]
  );

  const initialValues = useMemo<SchemaFormValues>(
    () => ({
      title: mode === 'edit' ? (model?.title ?? '') : '',
      description: mode === 'edit' ? (model?.description ?? '') : '',
      code: mode === 'edit' ? (model?.code ?? '') : '',
      api_open:
        mode === 'edit'
          ? isApiOpen(model?.status)
          : isApiOpen(source?.default_data_model_status ?? 'published')
    }),
    [mode, model, source]
  );

  const templateUnavailable =
    mode === 'create' &&
    (templatesLoading || Boolean(templatesError) || !selectedTemplate);

  const templateSelector =
    mode === 'create' ? (
      <Space orientation="vertical" size={6} style={{ width: '100%' }}>
        <Typography.Text>
          {i18nText('settings', 'auto.data_model_template')}
        </Typography.Text>
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
          placeholder={i18nText('settings', 'auto.select_data_model_template')}
          options={compatibleTemplates.map((template) => ({
            value: dataModelTemplateIdentity(template),
            label: dataModelTemplatePresentation(template).title
          }))}
          optionRender={(option) => <span>{option.data.label}</span>}
          onChange={(identity) =>
            setSelectedTemplate(
              compatibleTemplates.find(
                (template) => dataModelTemplateIdentity(template) === identity
              ) ?? null
            )
          }
        />
      </Space>
    ) : undefined;

  const defaultFieldsPreview =
    mode === 'create' && selectedTemplate ? (
      <Space orientation="vertical" size={6} style={{ width: '100%' }}>
        <Typography.Text strong>
          {i18nText('settings', 'auto.default_fields')}
        </Typography.Text>
        <Table
          rowKey="code"
          size="small"
          columns={systemFieldColumns}
          dataSource={selectedTemplate.system_fields}
          pagination={false}
          scroll={{ x: 'max-content' }}
        />
      </Space>
    ) : undefined;

  const statusMessages =
    mode === 'create'
      ? [
          ...(templatesError
            ? [
                {
                  key: 'templates-error',
                  message: templatesError,
                  type: 'error' as const
                }
              ]
            : []),
          ...(!templatesLoading &&
          !templatesError &&
          compatibleTemplates.length === 0
            ? [
                {
                  key: 'templates-empty',
                  message: i18nText(
                    'settings',
                    'auto.no_compatible_data_model_template'
                  ),
                  type: 'warning' as const
                }
              ]
            : [])
        ]
      : [];

  return (
    <SchemaFormDrawer
      open={open}
      title={
        mode === 'create'
          ? i18nText('settings', 'auto.new_data_model')
          : i18nText('settings', 'auto.edit_data_model')
      }
      width={560}
      schema={schema}
      initialValues={initialValues}
      leadingContent={templateSelector}
      trailingContent={defaultFieldsPreview}
      statusMessages={statusMessages}
      submitDisabled={templateUnavailable}
      submitting={saving}
      submitText={
        mode === 'create'
          ? i18nText('settings', 'auto.create')
          : i18nText('settings', 'auto.save')
      }
      onCancel={onClose}
      onSubmit={(values) => {
        const title = values.title as string;
        const description = (values.description as string | undefined) ?? '';
        const apiOpen = values.api_open === true;

        if (mode === 'edit' && model) {
          onUpdate(model, {
            title,
            description: description.trim() || null,
            status: apiOpenStatus(apiOpen)
          });
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
          code: values.code as string,
          title,
          description: description.trim() || null,
          status: apiOpenStatus(apiOpen)
        });
      }}
      onSubmitSuccess={onClose}
    />
  );
}
