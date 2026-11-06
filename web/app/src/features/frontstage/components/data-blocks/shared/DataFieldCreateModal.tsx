import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Checkbox, Form, Input, Select, Space } from 'antd';
import { DataBlockModal as Modal } from './DataBlockModal';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  createConsoleDataModelField,
  fetchConsoleDataModels,
  type CreateConsoleDataModelFieldInput
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';
import { i18nText } from '../../../../../shared/i18n/text';
import { dataBlockMetadataKey } from '../../../api/data-blocks';
import { getFrontstageApiBaseUrl } from '../../../api/page-tree';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import type { DataBlockBinding } from '../../../lib/data-blocks/types';

interface Values {
  code: string;
  title: string;
  description?: string;
  field_kind: string;
  is_required?: boolean;
  is_unique?: boolean;
  display_interface?: string;
  external_field_key?: string;
  options?: Array<{ value: string; label: string }>;
  relation_target_model_id?: string;
}

export function DataFieldCreateModal({
  ctx,
  binding,
  open,
  onClose,
  onCreated
}: {
  ctx: BlockContext;
  binding: DataBlockBinding;
  open: boolean;
  onClose(): void;
  onCreated(fieldCode: string): void | Promise<void>;
}) {
  const [form] = Form.useForm<Values>();
  const kind = Form.useWatch('field_kind', form) ?? 'string';
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createdFieldCode, setCreatedFieldCode] = useState<string | null>(null);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const models = useQuery({
    queryKey: [
      'frontstage-design-models',
      ctx.workspace.id,
      ctx.currentUser?.id
    ],
    queryFn: () => fetchConsoleDataModels({}, getFrontstageApiBaseUrl()),
    enabled: open,
    retry: false
  });
  const model = models.data?.find((item) => item.id === binding.model_id);
  const allowed = Boolean(
    model?.capabilities.can_add_user_field &&
    (ctx.currentUser?.isRoot ||
      ctx.currentUser?.permissions?.includes('model_fields.create'))
  );
  const save = async () => {
    const values = await form.validateFields();
    if (!allowed || !csrfToken || saving) return;
    setError(null);
    setSaving(true);
    try {
      const input: CreateConsoleDataModelFieldInput = {
        code: values.code,
        title: values.title,
        description: values.description || null,
        field_kind: values.field_kind,
        is_required: Boolean(values.is_required),
        is_unique: Boolean(values.is_unique),
        default_value: null,
        external_field_key: values.external_field_key || null,
        display_interface:
          values.field_kind === 'enum'
            ? (values.display_interface ?? 'select')
            : null,
        display_options:
          values.field_kind === 'enum' ? { options: values.options ?? [] } : {},
        relation_target_model_id: values.relation_target_model_id || null,
        relation_options: {}
      };
      if (!createdFieldCode) {
        await createConsoleDataModelField(
          binding.model_id,
          input,
          csrfToken,
          getFrontstageApiBaseUrl()
        );
        setCreatedFieldCode(values.code);
      }
      await queryClient.invalidateQueries({
        queryKey: dataBlockMetadataKey(ctx, binding.model_code)
      });
      await models.refetch();
      await onCreated(createdFieldCode ?? values.code);
      setCreatedFieldCode(null);
      form.resetFields();
      onClose();
    } catch (failure) {
      setError(dataBlockError(failure));
    } finally {
      setSaving(false);
    }
  };
  return (
    <Modal
      open={open}
      getContainer={false}
      destroyOnHidden
      title={i18nText('frontstage', 'data_blocks.fields.create')}
      onCancel={() => {
        if (!saving) {
          setError(null);
          setCreatedFieldCode(null);
          form.resetFields();
          onClose();
        }
      }}
      onOk={() => {
        save().catch(() => {});
      }}
      confirmLoading={saving}
      okButtonProps={{ disabled: !allowed || models.isLoading }}
    >
      {error || models.error ? (
        <Alert
          type="error"
          showIcon
          title={
            createdFieldCode
              ? i18nText(
                  'frontstage',
                  'data_blocks.fields.created_attach_failed'
                )
              : (error ?? dataBlockError(models.error))
          }
          description={createdFieldCode ? error : undefined}
        />
      ) : null}
      {!models.isLoading && !allowed && !models.error ? (
        <Alert
          type="info"
          title={i18nText('frontstage', 'data_blocks.fields.not_allowed')}
        />
      ) : null}
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          field_kind: 'string',
          options: [{ label: '', value: '' }]
        }}
        disabled={saving || !allowed || Boolean(createdFieldCode)}
      >
        <Form.Item
          name="title"
          label={i18nText('frontstage', 'data_blocks.fields.title')}
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="code"
          label={i18nText('frontstage', 'data_blocks.fields.code')}
          rules={[{ required: true }, { pattern: /^[A-Za-z][A-Za-z0-9_]*$/ }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="field_kind"
          label={i18nText('frontstage', 'data_blocks.fields.type')}
        >
          <Select
            options={[
              {
                value: 'string',
                label: i18nText('frontstage', 'data_blocks.fields.text')
              },
              {
                value: 'text',
                label: i18nText('frontstage', 'data_blocks.fields.long_text')
              },
              {
                value: 'number',
                label: i18nText('frontstage', 'data_blocks.fields.number')
              },
              {
                value: 'boolean',
                label: i18nText('frontstage', 'data_blocks.fields.boolean')
              },
              {
                value: 'datetime',
                label: i18nText('frontstage', 'data_blocks.fields.datetime')
              },
              {
                value: 'enum',
                label: i18nText('frontstage', 'data_blocks.fields.select')
              },
              {
                value: 'many_to_one',
                label: i18nText('frontstage', 'data_blocks.fields.many_to_one')
              },
              {
                value: 'many_to_many',
                label: i18nText('frontstage', 'data_blocks.fields.many_to_many')
              }
            ]}
          />
        </Form.Item>
        {model?.source_kind === 'external_source' ? (
          <Form.Item
            name="external_field_key"
            label={i18nText('frontstage', 'data_blocks.fields.external_key')}
            rules={[{ required: true }]}
          >
            <Input />
          </Form.Item>
        ) : null}
        {kind === 'enum' ? (
          <>
            <Form.Item
              name="display_interface"
              label={i18nText('frontstage', 'data_blocks.fields.display')}
              initialValue="select"
            >
              <Select
                options={[
                  {
                    value: 'select',
                    label: i18nText(
                      'frontstage',
                      'data_blocks.fields.single_select'
                    )
                  },
                  {
                    value: 'multi_select',
                    label: i18nText(
                      'frontstage',
                      'data_blocks.fields.multiple_select'
                    )
                  }
                ]}
              />
            </Form.Item>
            <Form.List name="options">
              {(items, { add, remove }) => (
                <>
                  {items.map((item) => (
                    <Space key={item.key} align="baseline">
                      <Form.Item
                        name={[item.name, 'value']}
                        rules={[{ required: true }]}
                      >
                        <Input
                          placeholder={i18nText(
                            'frontstage',
                            'data_blocks.fields.option_value'
                          )}
                        />
                      </Form.Item>
                      <Form.Item
                        name={[item.name, 'label']}
                        rules={[{ required: true }]}
                      >
                        <Input
                          placeholder={i18nText(
                            'frontstage',
                            'data_blocks.fields.option_label'
                          )}
                        />
                      </Form.Item>
                      <Button onClick={() => remove(item.name)}>×</Button>
                    </Space>
                  ))}
                  <Button
                    type="dashed"
                    onClick={() => add({ value: '', label: '' })}
                  >
                    {i18nText('frontstage', 'data_blocks.fields.add_option')}
                  </Button>
                </>
              )}
            </Form.List>
          </>
        ) : null}
        {kind === 'many_to_one' || kind === 'many_to_many' ? (
          <Form.Item
            name="relation_target_model_id"
            label={i18nText('frontstage', 'data_blocks.fields.related_model')}
            rules={[{ required: true }]}
          >
            <Select
              options={(models.data ?? []).map((item) => ({
                value: item.id,
                label: item.title
              }))}
            />
          </Form.Item>
        ) : null}
        <Form.Item
          name="description"
          label={i18nText('frontstage', 'auto.description')}
        >
          <Input.TextArea />
        </Form.Item>
        <Space>
          <Form.Item name="is_required" valuePropName="checked">
            <Checkbox>
              {i18nText('frontstage', 'data_blocks.fields.required')}
            </Checkbox>
          </Form.Item>
          <Form.Item name="is_unique" valuePropName="checked">
            <Checkbox>
              {i18nText('frontstage', 'data_blocks.fields.unique')}
            </Checkbox>
          </Form.Item>
        </Space>
      </Form>
    </Modal>
  );
}
