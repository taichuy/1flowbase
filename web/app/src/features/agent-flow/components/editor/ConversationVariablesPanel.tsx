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
  conversationVariableValueTypeOptions,
  formatConversationVariableTitle,
  type AgentFlowConversationVariable
} from '../../lib/variables/conversation-variables';
import { i18nText } from '../../../../shared/i18n/text';

interface ConversationVariableFormValues {
  name: string;
  valueType: string;
  description?: string;
}

interface ConversationVariablesPanelProps {
  variables: AgentFlowConversationVariable[];
  onClose: () => void;
  onSave: (variables: AgentFlowConversationVariable[]) => void;
}

export function ConversationVariablesPanel({
  variables,
  onClose,
  onSave
}: ConversationVariablesPanelProps) {
  const [draftVariables, setDraftVariables] = useState(variables);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [form] = Form.useForm<ConversationVariableFormValues>();

  useEffect(() => {
    setDraftVariables(variables);
  }, [variables]);

  const editingVariable =
    editingIndex === null ? null : draftVariables[editingIndex];
  const modalTitle = editingVariable
    ? i18nText('agentFlow', 'auto.edit_conversation_variable')
    : i18nText('agentFlow', 'auto.add_conversation_variable');

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
    form.setFieldsValue({
      name: '',
      valueType: 'string',
      description: ''
    });
    setModalOpen(true);
  }

  function openEditModal(index: number) {
    const variable = draftVariables[index];
    setEditingIndex(index);
    form.setFieldsValue({
      name: variable.name,
      valueType: variable.valueType,
      description: variable.description
    });
    setModalOpen(true);
  }

  async function submitModal() {
    const values = await form.validateFields();
    const nextVariable: AgentFlowConversationVariable = {
      name: values.name,
      valueType: values.valueType,
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
      aria-label={i18nText('agentFlow', 'auto.conversation_variables')}
      className="agent-flow-editor__environment-variables-panel"
    >
      <header className="agent-flow-editor__system-variables-header">
        <div className="agent-flow-editor__system-variables-heading">
          <Typography.Title level={3}>
            {i18nText('agentFlow', 'auto.conversation_variables')}
          </Typography.Title>
          <Typography.Text type="secondary">
            {i18nText(
              'agentFlow',
              'auto.conversation_variables_runtime_read_write'
            )}
          </Typography.Text>
        </div>
        <Button
          aria-label={i18nText(
            'agentFlow',
            'auto.turn_off_conversation_variables'
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
            {i18nText('agentFlow', 'auto.add_conversation_variable')}
          </Button>
        </div>
        <div className="agent-flow-editor__environment-variable-list">
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
                      {formatConversationVariableTitle(variable.name)}
                    </Typography.Text>
                    <Typography.Text type="secondary">
                      {variable.valueType}
                    </Typography.Text>
                  </div>
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
                      'auto.delete_conversation_variable'
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
            name="valueType"
            label={i18nText('agentFlow', 'auto.type')}
            rules={[
              {
                required: true,
                message: i18nText('agentFlow', 'auto.please_select_type')
              }
            ]}
          >
            <Select options={conversationVariableValueTypeOptions} />
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
