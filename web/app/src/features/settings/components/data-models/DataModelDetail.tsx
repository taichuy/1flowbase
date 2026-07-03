import { useState } from 'react';

import {
  Button,
  Flex,
  Table,
  Tabs,
  Tag,
  Typography,
  Space
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import {
  LockOutlined,
  PlusOutlined,
  EditOutlined
} from '@ant-design/icons';

import type {
  CreateSettingsDataModelFieldInput,
  SettingsDataModel,
  SettingsDataModelAdvisorFinding,
  SettingsDataModelField,
  SettingsDataModelScopeGrant,
  UpdateSettingsDataModelFieldInput,
  UpdateSettingsDataModelInput
} from '../../api/data-models';
import { DataModelAdvisorTab } from './DataModelAdvisorTab';
import { DataModelApiTab } from './DataModelApiTab';
import { DataModelFieldDrawer } from './DataModelFieldDrawer';
import { DataModelFormDrawer } from './DataModelFormDrawer';
import { DataModelPermissionsTab } from './DataModelPermissionsTab';
import { i18nText } from '../../../../shared/i18n/text';

function getFieldKindTag(kind: string) {
  let color = 'default';

  const k = kind.toLowerCase();
  if (k.includes('string') || k.includes('text') || k.includes('varchar')) {
    color = 'blue';
  } else if (
    k.includes('int') ||
    k.includes('float') ||
    k.includes('number') ||
    k.includes('decimal')
  ) {
    color = 'cyan';
  } else if (k.includes('bool')) {
    color = 'purple';
  } else if (k.includes('date') || k.includes('time')) {
    color = 'orange';
  } else if (k.includes('relation') || k.includes('ref')) {
    color = 'magenta';
  } else if (k.includes('json')) {
    color = 'geekblue';
  }

  return (
    <Tag
      color={color}
      style={{ borderRadius: 6, margin: 0, textTransform: 'capitalize' }}
    >
      {kind}
    </Tag>
  );
}

function apiAccessLabel(model: SettingsDataModel) {
  return model.status === 'published'
    ? i18nText('settings', 'auto.open')
    : i18nText('settings', 'auto.closed');
}

export function DataModelDetail({
  model,
  allModels,
  canManage,
  grants,
  grantsLoading,
  grantsSaving,
  advisorFindings,
  advisorLoading,
  modelSaving,
  fieldSaving,
  onUpdateModel,
  onCreateField,
  onUpdateField,
  onDeleteField,
  onSaveGrant
}: {
  model: SettingsDataModel;
  allModels: SettingsDataModel[];
  canManage: boolean;
  grants: SettingsDataModelScopeGrant[];
  grantsLoading: boolean;
  grantsSaving: boolean;
  advisorFindings: SettingsDataModelAdvisorFinding[];
  advisorLoading: boolean;
  modelSaving: boolean;
  fieldSaving: boolean;
  onUpdateModel: (input: UpdateSettingsDataModelInput) => void;
  onCreateField: (input: CreateSettingsDataModelFieldInput) => void;
  onUpdateField: (
    field: SettingsDataModelField,
    input: UpdateSettingsDataModelFieldInput
  ) => void;
  onDeleteField: (field: SettingsDataModelField) => void;
  onSaveGrant: Parameters<typeof DataModelPermissionsTab>[0]['onSave'];
}) {
  const [modelDrawerOpen, setModelDrawerOpen] = useState(false);
  const [fieldDrawerState, setFieldDrawerState] = useState<
    | { open: false; mode: 'create'; field: null }
    | { open: true; mode: 'create'; field: null }
    | { open: true; mode: 'edit'; field: SettingsDataModelField }
  >({ open: false, mode: 'create', field: null });

  const fieldColumns: ColumnsType<SettingsDataModelField> = [
    {
      title: i18nText("settings", "auto.field_title"),
      dataIndex: 'title',
      key: 'title',
      render: (value: string, field) => {
        const canUpdatePresentation =
          field.capabilities.can_update_presentation_metadata;
        return (
          <Space direction="vertical" size={2}>
            <Space size={8}>
              <button
                type="button"
                className={`data-model-panel__field-title-btn ${canUpdatePresentation ? '' : 'disabled'}`}
                disabled={!canUpdatePresentation}
                onClick={() =>
                  setFieldDrawerState({ open: true, mode: 'edit', field })
                }
              >
                <Typography.Text
                  strong={canUpdatePresentation}
                  style={{
                    color: canUpdatePresentation
                      ? 'var(--brand-primary)'
                      : 'var(--ant-color-text-secondary)'
                  }}
                >
                  {value}
                </Typography.Text>
              </button>
              {!canUpdatePresentation && (
                <Tag
                  style={{ borderRadius: 6, margin: 0, fontSize: 10 }}
                  icon={<LockOutlined style={{ fontSize: 10 }} />}
                >
                  {i18nText("settings", "auto.read_only")}</Tag>
              )}
            </Space>
          </Space>
        );
      }
    },
    {
      title: i18nText("settings", "auto.description"),
      dataIndex: 'description',
      key: 'description',
      render: (value: string | null) =>
        value ? (
          <Typography.Text type="secondary">{value}</Typography.Text>
        ) : (
          <span style={{ color: 'var(--text-tertiary)' }}>-</span>
        )
    },
    {
      title: 'Code',
      dataIndex: 'code',
      key: 'code',
      render: (value: string) => (
        <code className="data-model-panel__code-badge">{value}</code>
      )
    },
    {
      title: i18nText("settings", "auto.kind"),
      dataIndex: 'field_kind',
      key: 'field_kind',
      render: (value: string) => getFieldKindTag(value)
    },
    {
      title: i18nText("settings", "auto.required"),
      dataIndex: 'is_required',
      key: 'is_required',
      width: 80,
      render: (value: boolean) =>
        value ? (
          <Tag color="error" style={{ borderRadius: 4, margin: 0 }}>
            {i18nText("settings", "auto.required")}</Tag>
        ) : (
          <span style={{ color: 'var(--text-tertiary)' }}>-</span>
        )
    },
    {
      title: i18nText("settings", "auto.only"),
      dataIndex: 'is_unique',
      key: 'is_unique',
      width: 80,
      render: (value: boolean) =>
        value ? (
          <Tag color="warning" style={{ borderRadius: 4, margin: 0 }}>
            {i18nText("settings", "auto.only")}</Tag>
        ) : (
          <span style={{ color: 'var(--text-tertiary)' }}>-</span>
        )
    },
    {
      title: i18nText("settings", "auto.operation"),
      key: 'actions',
      width: 100,
      render: (_, field) => (
        <Button
          type="link"
          size="small"
          icon={<EditOutlined aria-hidden="true" />}
          style={{ padding: 0 }}
          disabled={
            !canManage ||
            !field.capabilities.can_update_presentation_metadata
          }
          onClick={() =>
            setFieldDrawerState({ open: true, mode: 'edit', field })
          }
        >
          {i18nText("settings", "auto.edit")}</Button>
      )
    }
  ];

  const relationFields = model.fields.filter(
    (field) => field.relation_target_model_id
  );
  const summaryRows = [
    [
      {
        key: 'title',
        label: i18nText("settings", "auto.title"),
        value: model.title
      },
      {
        key: 'source',
        label: i18nText("settings", "auto.source"),
        value: model.source_kind === 'main_source' ? i18nText("settings", "auto.built_in_data_source") : i18nText("settings", "auto.external_data_source")
      }
    ],
    [
      { key: 'code', label: 'Code', value: model.code },
      {
        key: 'runtime',
        label: i18nText('settings', 'auto.open_api'),
        value: apiAccessLabel(model)
      }
    ],
    [
      {
        key: 'table',
        label: i18nText("settings", "auto.physical_table"),
        value: model.physical_table_name
      }
    ],
    ...(model.source_kind === 'external_source'
      ? [
          [
            {
              key: 'external_table_id',
              label: i18nText("settings", "auto.table_id_alt"),
              value: model.external_table_id ?? '-'
            }
          ]
        ]
      : [])
  ];

  return (
    <section className="data-model-panel__detail" aria-label={i18nText("settings", "auto.data_model_details")}>
      <div
        className="data-model-panel__meta-grid"
        data-testid="data-model-detail-summary"
      >
        {summaryRows.map((row, index) => (
          <div
            key={`summary-row-${index}`}
            className="data-model-panel__meta-row"
            data-testid="data-model-summary-row"
          >
            {row.map((item) => (
              <div
                key={item.key}
                className="data-model-panel__meta-card"
                data-testid="data-model-summary-item"
              >
                <span className="data-model-panel__meta-card-label">
                  {item.label}：
                </span>
                <Typography.Text
                  strong
                  className="data-model-panel__meta-card-value"
                >
                  {item.value}
                </Typography.Text>
              </div>
            ))}
          </div>
        ))}
      </div>

      <div
        className="data-model-panel__detail-actions"
        data-testid="data-model-detail-actions"
      >
        <Button disabled={!canManage} onClick={() => setModelDrawerOpen(true)}>
          {i18nText("settings", "auto.edit")}</Button>
      </div>

      <Tabs
        items={[
          {
            key: 'fields',
            label: i18nText("settings", "auto.field"),
            children: (
              <Flex vertical gap={12}>
                <Flex justify="flex-end">
                  <Button
                    type="primary"
                    icon={<PlusOutlined aria-hidden="true" />}
                    disabled={
                      !canManage || !model.capabilities.can_add_user_field
                    }
                    onClick={() =>
                      setFieldDrawerState({
                        open: true,
                        mode: 'create',
                        field: null
                      })
                    }
                  >
                    {i18nText("settings", "auto.add_new_field")}</Button>
                </Flex>
                <Table
                  rowKey="id"
                  size="small"
                  columns={fieldColumns}
                  dataSource={model.fields}
                  pagination={false}
                />
                <DataModelFieldDrawer
                  open={fieldDrawerState.open}
                  mode={fieldDrawerState.mode}
                  field={fieldDrawerState.field}
                  isExternalModel={model.source_kind === 'external_source'}
                  modelOptions={allModels}
                  saving={fieldSaving}
                  canManage={canManage}
                  onClose={() =>
                    setFieldDrawerState({
                      open: false,
                      mode: 'create',
                      field: null
                    })
                  }
                  onCreate={onCreateField}
                  onUpdate={onUpdateField}
                  onDelete={onDeleteField}
                />
              </Flex>
            )
          },
          {
            key: 'relations',
            label: i18nText("settings", "auto.relationship"),
            children: (
              <Table
                rowKey="id"
                size="small"
                columns={[
                  { title: i18nText("settings", "auto.field"), dataIndex: 'title', key: 'title' },
                  {
                    title: i18nText("settings", "auto.target_model"),
                    dataIndex: 'relation_target_model_id',
                    key: 'relation_target_model_id'
                  }
                ]}
                dataSource={relationFields}
                pagination={false}
              />
            )
          },
          {
            key: 'permissions',
            label: i18nText("settings", "auto.permissions_alt"),
            children: (
              <DataModelPermissionsTab
                grants={grants}
                loading={grantsLoading}
                saving={grantsSaving}
                onSave={onSaveGrant}
              />
            )
          },
          {
            key: 'api',
            label: 'API',
            children: (
              <DataModelApiTab
                model={model}
                canManage={canManage}
                saving={fieldSaving}
                onUpdateApiRequired={(field, apiRequired) =>
                  onUpdateField(field, {
                    title: field.title,
                    description: field.description,
                    is_required: field.is_required,
                    api_required: apiRequired,
                    is_unique: field.is_unique,
                    default_value: field.default_value,
                    display_interface: field.display_interface,
                    display_options: field.display_options,
                    relation_options: field.relation_options
                  })
                }
              />
            )
          },
          {
            key: 'advisor',
            label: i18nText("settings", "auto.risk_warnings"),
            children: (
              <DataModelAdvisorTab
                findings={advisorFindings}
                loading={advisorLoading}
              />
            )
          }
        ]}
      />
      <DataModelFormDrawer
        open={modelDrawerOpen}
        mode="edit"
        model={model}
        source={null}
        saving={modelSaving}
        onClose={() => setModelDrawerOpen(false)}
        onCreate={() => undefined}
        onUpdate={(_model, input) => {
          onUpdateModel(input);
          setModelDrawerOpen(false);
        }}
      />
    </section>
  );
}
