import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import CodeOutlined from '@ant-design/icons/es/icons/CodeOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import {
  Button,
  Empty,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Tooltip,
  Typography
} from 'antd';
import { useEffect, useMemo, useState } from 'react';

import {
  formatEnvironmentVariableTitle,
  type AgentFlowEnvironmentVariable
} from '../../lib/variables/application-environment-variables';
import { EnvironmentVariableValueEditor } from './environment-variables/EnvironmentVariableValueEditor';
import { i18nText } from '../../../../shared/i18n/text';

const valueTypeOptions = [
  'string',
  'number',
  'boolean',
  'object',
  'array[string]',
  'array[number]',
  'array[boolean]',
  'array[object]'
].map((value) => ({ label: value, value }));

interface EnvironmentVariableFormValues {
  name: string;
  value_type: string;
  value?:
    | string
    | number
    | boolean
    | Record<string, unknown>
    | unknown[]
    | null;
  description?: string;
}

interface ApplicationEnvironmentVariablesPanelProps {
  variables: AgentFlowEnvironmentVariable[];
  loading?: boolean;
  onClose: () => void;
  onSave: (variables: AgentFlowEnvironmentVariable[]) => void;
}

function formatVariableValue(value: unknown) {
  if (typeof value === 'string') {
    return value;
  }

  return JSON.stringify(value, null, 2);
}

function formatFormValue(
  valueType: string,
  value: unknown
): EnvironmentVariableFormValues['value'] {
  if (valueType === 'string') {
    return typeof value === 'string' ? value : '';
  }

  if (valueType === 'number') {
    return typeof value === 'number' ? value : null;
  }

  if (valueType === 'boolean') {
    return typeof value === 'boolean' ? value : false;
  }

  if (
    valueType === 'object' &&
    Boolean(value && typeof value === 'object' && !Array.isArray(value))
  ) {
    return value as Record<string, unknown>;
  }

  if (valueType.startsWith('array[') && Array.isArray(value)) {
    return value;
  }

  return createDefaultValueForType(valueType);
}

function createDefaultValueForType(valueType: string) {
  if (valueType === 'boolean') {
    return false;
  }

  if (valueType === 'object') {
    return {};
  }

  if (valueType.startsWith('array[')) {
    return [];
  }

  return '';
}

function parseVariableValue(valueType: string, rawValue: unknown) {
  if (valueType === 'string') {
    return typeof rawValue === 'string' ? rawValue : '';
  }

  if (valueType === 'number') {
    const parsed =
      typeof rawValue === 'number' ? rawValue : Number(String(rawValue));
    if (!Number.isFinite(parsed)) {
      throw new Error('value');
    }
    return parsed;
  }

  if (valueType === 'boolean') {
    if (rawValue === true || rawValue === 'true') {
      return true;
    }
    if (rawValue === false || rawValue === 'false') {
      return false;
    }
    throw new Error('value');
  }

  if (valueType === 'object') {
    if (rawValue && typeof rawValue === 'object' && !Array.isArray(rawValue)) {
      return rawValue;
    }

    throw new Error('value');
  }

  if (valueType.startsWith('array[')) {
    if (Array.isArray(rawValue)) {
      return rawValue;
    }

    throw new Error('value');
  }

  throw new Error('value');
}

function validateParsedValue(valueType: string, value: unknown) {
  const valid =
    (valueType === 'object' &&
      Boolean(value && typeof value === 'object' && !Array.isArray(value))) ||
    (valueType === 'array[string]' &&
      Array.isArray(value) &&
      value.every((item) => typeof item === 'string')) ||
    (valueType === 'array[number]' &&
      Array.isArray(value) &&
      value.every((item) => typeof item === 'number')) ||
    (valueType === 'array[boolean]' &&
      Array.isArray(value) &&
      value.every((item) => typeof item === 'boolean')) ||
    (valueType === 'array[object]' &&
      Array.isArray(value) &&
      value.every(
        (item) =>
          Boolean(item && typeof item === 'object') && !Array.isArray(item)
      ));

  if (['string', 'number', 'boolean'].includes(valueType) || valid) {
    return;
  }

  throw new Error('value');
}

export function ApplicationEnvironmentVariablesPanel({
  variables,
  loading = false,
  onClose,
  onSave
}: ApplicationEnvironmentVariablesPanelProps) {
  const [draftVariables, setDraftVariables] = useState(variables);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [valueError, setValueError] = useState<string | null>(null);
  const [form] = Form.useForm<EnvironmentVariableFormValues>();
  const selectedValueType = Form.useWatch('value_type', form) ?? 'string';

  useEffect(() => {
    setDraftVariables(variables);
  }, [variables]);

  const editingVariable =
    editingIndex === null ? null : draftVariables[editingIndex];
  const modalTitle = editingVariable
    ? i18nText('agentFlow', 'auto.edit_environment_variables')
    : i18nText('agentFlow', 'auto.add_environment_variables');

  const existingNames = useMemo(
    () =>
      new Set(
        draftVariables
          .filter((_, index) => index !== editingIndex)
          .map((variable) => variable.name)
      ),
    [draftVariables, editingIndex]
  );

  function openCreateModal() {
    setEditingIndex(null);
    setValueError(null);
    form.setFieldsValue({
      name: '',
      value_type: 'string',
      value: createDefaultValueForType('string'),
      description: ''
    });
    setModalOpen(true);
  }

  function openEditModal(index: number) {
    const variable = draftVariables[index];
    setEditingIndex(index);
    setValueError(null);
    form.setFieldsValue({
      name: variable.name,
      value_type: variable.value_type,
      value: formatFormValue(variable.value_type, variable.value),
      description: variable.description
    });
    setModalOpen(true);
  }

  function handleValueTypeChange(valueType: string) {
    setValueError(null);
    form.setFieldsValue({
      value_type: valueType,
      value: createDefaultValueForType(valueType)
    });
  }

  async function submitModal() {
    const values = await form.validateFields();
    let parsedValue: unknown;

    try {
      parsedValue = parseVariableValue(values.value_type, values.value);
      validateParsedValue(values.value_type, parsedValue);
    } catch {
      setValueError(i18nText('agentFlow', 'auto.variable_value_match_type'));
      return;
    }

    const nextVariable: AgentFlowEnvironmentVariable = {
      name: values.name,
      value_type: values.value_type,
      value: parsedValue,
      description: values.description?.trim() ?? ''
    };
    const nextVariables =
      editingIndex === null
        ? [...draftVariables, nextVariable]
        : draftVariables.map((variable, index) =>
            index === editingIndex ? nextVariable : variable
          );

    setDraftVariables(nextVariables);
    onSave(nextVariables);
    setModalOpen(false);
  }

  function deleteVariable(index: number) {
    const nextVariables = draftVariables.filter(
      (_, candidate) => candidate !== index
    );
    setDraftVariables(nextVariables);
    onSave(nextVariables);
  }

  return (
    <section
      aria-label={i18nText('agentFlow', 'auto.environment_variables')}
      className="agent-flow-editor__environment-variables-panel"
    >
      <header className="agent-flow-editor__system-variables-header">
        <div className="agent-flow-editor__system-variables-heading">
          <Typography.Title level={3}>
            {i18nText('agentFlow', 'auto.environment_variables')}
          </Typography.Title>
          <Typography.Text type="secondary">
            {i18nText(
              'agentFlow',
              'auto.environment_variables_belong_application_referenced_any_node_canvas_through_env'
            )}
          </Typography.Text>
        </div>
        <Button
          aria-label={i18nText(
            'agentFlow',
            'auto.turn_off_environment_variables'
          )}
          icon={<CloseOutlined />}
          type="text"
          onClick={onClose}
        />
      </header>
      <div className="agent-flow-editor__environment-variables-body">
        <div className="agent-flow-editor__environment-variables-toolbar">
          <Button
            icon={<PlusOutlined />}
            type="primary"
            onClick={openCreateModal}
          >
            {i18nText('agentFlow', 'auto.add_environment_variables')}
          </Button>
        </div>
        <div
          aria-busy={loading}
          className="agent-flow-editor__environment-variable-list"
        >
          {draftVariables.length === 0 ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="No data" />
          ) : (
            draftVariables.map((variable, index) => (
              <div
                className="agent-flow-editor__environment-variable-row"
                key={variable.name}
              >
                <CodeOutlined className="agent-flow-editor__environment-variable-icon" />
                <div className="agent-flow-editor__environment-variable-content">
                  <div className="agent-flow-editor__environment-variable-title">
                    <Typography.Text strong>
                      {formatEnvironmentVariableTitle(variable.name)}
                    </Typography.Text>
                    <Typography.Text type="secondary">
                      {variable.value_type}
                    </Typography.Text>
                  </div>
                  <Typography.Text
                    className="agent-flow-editor__environment-variable-value"
                    type="secondary"
                  >
                    {formatVariableValue(variable.value)}
                  </Typography.Text>
                  {variable.description ? (
                    <Typography.Text
                      className="agent-flow-editor__environment-variable-description"
                      type="secondary"
                    >
                      {variable.description}
                    </Typography.Text>
                  ) : null}
                </div>
                <Space size={2}>
                  <Tooltip title={i18nText('agentFlow', 'auto.edit_alt')}>
                    <Button
                      aria-label={i18nText('agentFlow', 'auto.edit', {
                        value1: variable.name
                      })}
                      icon={<EditOutlined />}
                      size="small"
                      type="text"
                      onClick={() => openEditModal(index)}
                    />
                  </Tooltip>
                  <Popconfirm
                    title={i18nText(
                      'agentFlow',
                      'auto.delete_environment_variables'
                    )}
                    okText={i18nText('agentFlow', 'auto.delete')}
                    cancelText={i18nText('agentFlow', 'auto.cancel')}
                    onConfirm={() => deleteVariable(index)}
                  >
                    <Tooltip title={i18nText('agentFlow', 'auto.delete')}>
                      <Button
                        aria-label={i18nText('agentFlow', 'auto.delete_item', {
                          value1: variable.name
                        })}
                        danger
                        icon={<DeleteOutlined />}
                        size="small"
                        type="text"
                      />
                    </Tooltip>
                  </Popconfirm>
                </Space>
              </div>
            ))
          )}
        </div>
      </div>
      <Modal
        title={modalTitle}
        open={modalOpen}
        confirmLoading={loading}
        okText={i18nText('agentFlow', 'auto.save')}
        cancelText={i18nText('agentFlow', 'auto.cancel')}
        width={420}
        onCancel={() => setModalOpen(false)}
        onOk={() => {
          void submitModal();
        }}
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="name"
            label={i18nText('agentFlow', 'auto.name')}
            rules={[
              {
                required: true,
                message: i18nText('agentFlow', 'auto.enter_variable_name')
              },
              {
                pattern: /^[A-Za-z][A-Za-z0-9]*$/,
                message: i18nText(
                  'agentFlow',
                  'auto.supports_letters_starting_letters_including_uppercase_lowercase_letters_numbers'
                )
              },
              {
                validator(_, value) {
                  if (value && existingNames.has(value)) {
                    return Promise.reject(
                      new Error(
                        i18nText(
                          'agentFlow',
                          'auto.variable_name_already_exists'
                        )
                      )
                    );
                  }
                  return Promise.resolve();
                }
              }
            ]}
          >
            <Input placeholder="ApiBaseUrl" />
          </Form.Item>
          <Form.Item
            name="value_type"
            label={i18nText('agentFlow', 'auto.type')}
            rules={[
              {
                required: true,
                message: i18nText('agentFlow', 'auto.please_select_type')
              }
            ]}
          >
            <Select
              options={valueTypeOptions}
              onChange={handleValueTypeChange}
            />
          </Form.Item>
          <Form.Item
            name="value"
            label={i18nText('agentFlow', 'auto.value')}
            validateStatus={valueError ? 'error' : undefined}
            help={valueError ?? undefined}
            rules={[
              {
                validator(_, value) {
                  if (
                    value === undefined ||
                    value === null ||
                    (typeof value === 'string' && value.trim().length === 0)
                  ) {
                    return Promise.reject(
                      new Error(
                        i18nText('agentFlow', 'auto.enter_variable_value')
                      )
                    );
                  }

                  return Promise.resolve();
                }
              }
            ]}
          >
            <EnvironmentVariableValueEditor
              valueType={selectedValueType}
              onValueErrorChange={setValueError}
            />
          </Form.Item>
          <Form.Item
            name="description"
            label={i18nText('agentFlow', 'auto.description')}
          >
            <Input.TextArea autoSize={{ minRows: 2, maxRows: 4 }} />
          </Form.Item>
        </Form>
      </Modal>
    </section>
  );
}
