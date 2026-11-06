import { DataBlockModal as Modal } from '../shared/DataBlockModal';
import { useState, type ReactNode } from 'react';
import {
  Button,
  Checkbox,
  Divider,
  Dropdown,
  Form,
  Input,
  InputNumber,
  Radio,
  Select,
  Space,
  Switch,
  Typography
} from 'antd';
import {
  DeleteOutlined,
  PlusOutlined,
  SettingOutlined
} from '@ant-design/icons';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  emptyDataFilter,
  type DataBlockField,
  type DataFormItem,
  type DataFormRule,
  type FormBlockConfig
} from '../../../lib/data-blocks/types';
import { DataBlockFilterEditor } from '../shared/DataBlockFilterEditor';
import { DataRelationEditor } from '../shared/DataRelationEditor';
import { DataFieldCreateModal } from '../shared/DataFieldCreateModal';
import { FormFieldControl } from './FormFieldControl';

export type DesignDialog =
  | 'title'
  | 'layout'
  | 'rules'
  | 'actions'
  | 'relation'
  | DataFormItem
  | null;
type Props = {
  submitAction?: ReactNode;
  ctx: BlockContext;
  config: FormBlockConfig;
  fields: DataBlockField[];
  saving: boolean;
  onSave(config: FormBlockConfig): Promise<boolean>;
  onFieldCreated(code: string): Promise<void>;
  dialog: DesignDialog;
  setDialog(dialog: DesignDialog): void;
};
const popupContainer = (node: HTMLElement) => node.parentElement ?? node;
export function newFormItem(
  kind: DataFormItem['kind'],
  fieldCode?: string
): DataFormItem {
  const id = crypto.randomUUID();
  return { id, kind, field_code: fieldCode, row: id, span: 24 };
}

export function FormDesignControls(props: Props) {
  const { ctx, config, fields, saving, onSave, dialog, setDialog } = props;
  const [creatingField, setCreatingField] = useState(false);
  const [search, setSearch] = useState('');
  const fieldMenu = fields
    .filter((field) =>
      `${field.title} ${field.code}`
        .toLowerCase()
        .includes(search.toLowerCase())
    )
    .map((field) => ({
      key: field.code,
      label: (
        <span className="data-form-menu-option">
          <span>{field.title}</span>
          <Switch
            size="small"
            checked={config.items.some(
              (item) => item.field_code === field.code
            )}
            disabled={saving}
          />
        </span>
      ),
      onClick: () => {
        if (!saving)
          void onSave({
            ...config,
            items: config.items.some((item) => item.field_code === field.code)
              ? config.items.filter((item) => item.field_code !== field.code)
              : [...config.items, newFormItem('field', field.code)]
          });
      }
    }));
  return (
    <>
      <div className="data-form-block-tools">
        <Dropdown
          trigger={['click']}
          getPopupContainer={popupContainer}
          menu={{
            items: [
              {
                key: 'title',
                label: i18nText(
                  'frontstage',
                  'data_blocks.form.title_description'
                ),
                onClick: () => setDialog('title')
              },
              {
                key: 'rules',
                label: i18nText('frontstage', 'data_blocks.form.rules'),
                onClick: () => setDialog('rules')
              },
              {
                key: 'relation',
                label: i18nText(
                  'frontstage',
                  'data_blocks.form.relation_settings'
                ),
                onClick: () => setDialog('relation')
              },
              {
                key: 'layout',
                label: i18nText('frontstage', 'data_blocks.form.layout'),
                onClick: () => setDialog('layout')
              }
            ]
          }}
        >
          <Button
            size="small"
            icon={<SettingOutlined />}
            disabled={saving}
            aria-label={i18nText(
              'frontstage',
              'data_blocks.form.block_settings'
            )}
          />
        </Dropdown>
      </div>
      <div className="data-form-config-fields">
        <Dropdown
          trigger={['click']}
          getPopupContainer={popupContainer}
          menu={{
            items: [
              ...fieldMenu,
              { type: 'divider' },
              {
                key: '__markdown',
                label: i18nText('frontstage', 'data_blocks.form.add_markdown'),
                onClick: () => setDialog(newFormItem('markdown'))
              },
              {
                key: '__group',
                label: i18nText('frontstage', 'data_blocks.form.add_group'),
                onClick: () => setDialog(newFormItem('group'))
              },
              ...(ctx.currentUser?.permissions?.includes(
                'model_fields.create'
              ) || ctx.currentUser?.isRoot
                ? [
                    {
                      key: '__create',
                      label: i18nText(
                        'frontstage',
                        'data_blocks.form.create_field'
                      ),
                      onClick: () => setCreatingField(true)
                    }
                  ]
                : [])
            ]
          }}
          popupRender={(menu) => (
            <div className="data-form-field-menu">
              <Input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder={i18nText(
                  'frontstage',
                  'data_blocks.form.search_fields'
                )}
              />
              {menu}
            </div>
          )}
        >
          <Button type="dashed" icon={<SettingOutlined />} disabled={saving}>
            {i18nText('frontstage', 'data_blocks.form.configure_fields')}
          </Button>
        </Dropdown>
      </div>
      <div className="data-form-config-actions">
        {props.submitAction}
        <Dropdown
          trigger={['click']}
          getPopupContainer={popupContainer}
          menu={{
            items: [
              {
                key: 'submit',
                label: (
                  <span className="data-form-menu-option">
                    {i18nText('frontstage', 'data_blocks.form.submit')}
                    <Switch
                      size="small"
                      checked={config.submit_enabled}
                      disabled={saving}
                    />
                  </span>
                ),
                onClick: () => {
                  void onSave({
                    ...config,
                    submit_enabled: !config.submit_enabled
                  });
                }
              },
              {
                key: 'settings',
                label: i18nText(
                  'frontstage',
                  'data_blocks.form.action_settings'
                ),
                onClick: () => setDialog('actions')
              }
            ]
          }}
        >
          <Button type="dashed" icon={<SettingOutlined />} disabled={saving}>
            {i18nText('frontstage', 'data_blocks.form.configure_actions')}
          </Button>
        </Dropdown>
      </div>
      {dialog && (
        <FormSettingsModal
          key={typeof dialog === 'string' ? dialog : dialog.id}
          {...props}
          dialog={dialog}
        />
      )}
      <DataFieldCreateModal
        ctx={ctx}
        binding={config.binding}
        open={creatingField}
        onClose={() => setCreatingField(false)}
        onCreated={async (code) => {
          await props.onFieldCreated(code);
          setCreatingField(false);
        }}
      />
    </>
  );
}

function FormSettingsModal({
  ctx,
  config,
  fields,
  dialog,
  setDialog,
  saving,
  onSave
}: Props & { dialog: Exclude<DesignDialog, null> }) {
  const [draft, setDraft] = useState(config);
  const [item, setItem] = useState<DataFormItem | null>(
    typeof dialog === 'object' ? dialog : null
  );
  const patch = (values: Partial<FormBlockConfig>) =>
    setDraft((previous) => ({ ...previous, ...values }));
  const patchItem = (values: Partial<DataFormItem>) =>
    setItem((previous) => (previous ? { ...previous, ...values } : null));
  const field = fields.find((candidate) => candidate.code === item?.field_code);
  const title =
    dialog === 'relation'
      ? i18nText('frontstage', 'data_blocks.form.relation_settings')
      : item
        ? i18nText('frontstage', 'data_blocks.form.field_settings')
        : dialog === 'title'
          ? i18nText('frontstage', 'data_blocks.form.title_description')
          : dialog === 'layout'
            ? i18nText('frontstage', 'data_blocks.form.layout')
            : dialog === 'rules'
              ? i18nText('frontstage', 'data_blocks.form.rules')
              : i18nText('frontstage', 'data_blocks.form.action_settings');
  const save = async () => {
    const next = item
      ? {
          ...draft,
          items: draft.items.some((candidate) => candidate.id === item.id)
            ? draft.items.map((candidate) =>
                candidate.id === item.id ? item : candidate
              )
            : [...draft.items, item]
        }
      : draft;
    if (await onSave(next)) setDialog(null);
  };
  return (
    <Modal
      open
      title={title}
      getContainer={false}
      width={dialog === 'rules' ? 720 : 520}
      onCancel={() => setDialog(null)}
      onOk={() => void save()}
      confirmLoading={saving}
      okText={i18nText('frontstage', 'data_blocks.form.save_design')}
      cancelText={i18nText('frontstage', 'auto.cancel')}
    >
      <Form layout="vertical">
        {(dialog === 'title' || item) && (
          <>
            <Form.Item label={i18nText('frontstage', 'auto.title')}>
              <Input
                value={item ? (item.title ?? '') : draft.title}
                onChange={(event) =>
                  item
                    ? patchItem({ title: event.target.value })
                    : patch({ title: event.target.value })
                }
              />
            </Form.Item>
            <Form.Item label={i18nText('frontstage', 'auto.description')}>
              <Input.TextArea
                value={item ? (item.description ?? '') : draft.description}
                onChange={(event) =>
                  item
                    ? patchItem({ description: event.target.value })
                    : patch({ description: event.target.value })
                }
              />
            </Form.Item>
          </>
        )}
        {item && (
          <>
            {item.kind === 'markdown' && (
              <Form.Item
                label={i18nText('frontstage', 'data_blocks.form.markdown')}
              >
                <Input.TextArea
                  rows={6}
                  value={item.text ?? ''}
                  onChange={(event) => patchItem({ text: event.target.value })}
                />
              </Form.Item>
            )}
            {item.kind === 'field' && field && (
              <>
                <Form.Item
                  label={i18nText('frontstage', 'data_blocks.form.tooltip')}
                >
                  <Input
                    value={item.tooltip ?? ''}
                    onChange={(event) =>
                      patchItem({ tooltip: event.target.value })
                    }
                  />
                </Form.Item>
                <Form.Item>
                  <Checkbox
                    disabled={field.is_required || field.api_required}
                    checked={
                      item.required || field.is_required || field.api_required
                    }
                    onChange={(event) =>
                      patchItem({ required: event.target.checked })
                    }
                  >
                    {i18nText('frontstage', 'data_blocks.fields.required')}
                  </Checkbox>
                </Form.Item>
                <Form.Item>
                  <Checkbox
                    disabled={
                      !field.is_writable || field.field_kind === 'one_to_many'
                    }
                    checked={
                      item.read_only ||
                      !field.is_writable ||
                      field.field_kind === 'one_to_many'
                    }
                    onChange={(event) =>
                      patchItem({ read_only: event.target.checked })
                    }
                  >
                    {i18nText('frontstage', 'data_blocks.form.readonly')}
                  </Checkbox>
                </Form.Item>
                <Form.Item
                  label={i18nText(
                    'frontstage',
                    'data_blocks.form.default_value'
                  )}
                >
                  <FormFieldControl
                    ctx={ctx}
                    field={field}
                    value={item.default_value}
                    onChange={(default_value) => patchItem({ default_value })}
                    readOnly={
                      !field.is_writable || field.field_kind === 'one_to_many'
                    }
                  />
                </Form.Item>
              </>
            )}
            <Form.Item
              label={i18nText('frontstage', 'data_blocks.form.column_width')}
            >
              <Select
                value={item.span}
                getPopupContainer={popupContainer}
                options={[24, 18, 16, 12, 8, 6].map((span) => ({
                  value: span,
                  label: `${Math.round((span / 24) * 100)}%`
                }))}
                onChange={(span) => patchItem({ span })}
              />
            </Form.Item>
          </>
        )}
        {dialog === 'layout' && (
          <>
            <Form.Item>
              <Radio.Group
                value={draft.label_layout}
                onChange={(event) =>
                  patch({ label_layout: event.target.value })
                }
                options={[
                  {
                    value: 'vertical',
                    label: i18nText('frontstage', 'data_blocks.form.vertical')
                  },
                  {
                    value: 'horizontal',
                    label: i18nText('frontstage', 'data_blocks.form.horizontal')
                  }
                ]}
              />
            </Form.Item>
            {draft.label_layout === 'horizontal' && (
              <>
                <Form.Item
                  label={i18nText('frontstage', 'data_blocks.form.label_align')}
                >
                  <Select
                    getPopupContainer={popupContainer}
                    value={draft.label_align}
                    options={[
                      {
                        value: 'left',
                        label: i18nText('frontstage', 'data_blocks.form.left')
                      },
                      {
                        value: 'right',
                        label: i18nText('frontstage', 'data_blocks.form.right')
                      }
                    ]}
                    onChange={(label_align) => patch({ label_align })}
                  />
                </Form.Item>
                <Form.Item
                  label={i18nText('frontstage', 'data_blocks.form.label_width')}
                >
                  <InputNumber
                    min={40}
                    max={600}
                    suffix="px"
                    value={draft.label_width}
                    onChange={(value) => patch({ label_width: value ?? 120 })}
                  />
                </Form.Item>
                <Form.Item>
                  <Checkbox
                    checked={draft.label_wrap}
                    onChange={(event) =>
                      patch({ label_wrap: event.target.checked })
                    }
                  >
                    {i18nText('frontstage', 'data_blocks.form.label_wrap')}
                  </Checkbox>
                </Form.Item>
              </>
            )}
            <Checkbox
              checked={draft.colon}
              onChange={(event) => patch({ colon: event.target.checked })}
            >
              {i18nText('frontstage', 'data_blocks.form.colon')}
            </Checkbox>
          </>
        )}
        {dialog === 'actions' && (
          <>
            <Form.Item>
              <Checkbox
                checked={draft.submit_enabled}
                onChange={(event) =>
                  patch({ submit_enabled: event.target.checked })
                }
              >
                {i18nText('frontstage', 'data_blocks.form.submit_enabled')}
              </Checkbox>
            </Form.Item>
            <Form.Item
              label={i18nText('frontstage', 'data_blocks.form.submit_title')}
            >
              <Input
                value={draft.submit_title}
                onChange={(event) =>
                  patch({ submit_title: event.target.value })
                }
              />
            </Form.Item>
            <Checkbox
              checked={draft.close_after_submit}
              onChange={(event) =>
                patch({ close_after_submit: event.target.checked })
              }
            >
              {i18nText('frontstage', 'data_blocks.form.close_after_submit')}
            </Checkbox>
          </>
        )}
        {dialog === 'relation' && (
          <DataRelationEditor
            value={draft.relation}
            onChange={(relation) => patch({ relation })}
            fields={fields}
          />
        )}
        {dialog === 'rules' && (
          <>
            <Typography.Paragraph type="secondary">
              {i18nText('frontstage', 'data_blocks.form.rules_hint')}
            </Typography.Paragraph>
            {draft.rules.map((rule) => (
              <RuleEditor
                key={rule.id}
                rule={rule}
                fields={fields}
                onChange={(next) =>
                  patch({
                    rules: draft.rules.map((candidate) =>
                      candidate.id === rule.id ? next : candidate
                    )
                  })
                }
                onRemove={() =>
                  patch({
                    rules: draft.rules.filter(
                      (candidate) => candidate.id !== rule.id
                    )
                  })
                }
              />
            ))}
            <Button
              icon={<PlusOutlined />}
              onClick={() =>
                patch({
                  rules: [
                    ...draft.rules,
                    {
                      id: crypto.randomUUID(),
                      target_field: fields[0]?.code ?? '',
                      effect: 'show',
                      condition: emptyDataFilter()
                    }
                  ]
                })
              }
              disabled={!fields.length}
            >
              {i18nText('frontstage', 'data_blocks.form.add_rule')}
            </Button>
          </>
        )}
      </Form>
    </Modal>
  );
}

function RuleEditor({
  rule,
  fields,
  onChange,
  onRemove
}: {
  rule: DataFormRule;
  fields: DataBlockField[];
  onChange(rule: DataFormRule): void;
  onRemove(): void;
}) {
  return (
    <section className="data-form-rule">
      <DataBlockFilterEditor
        value={rule.condition}
        onChange={(condition) => onChange({ ...rule, condition })}
        fields={fields}
      />
      <Space wrap>
        <Select
          aria-label={i18nText('frontstage', 'data_blocks.form.target_field')}
          style={{ minWidth: 160 }}
          getPopupContainer={popupContainer}
          value={rule.target_field}
          options={fields.map((field) => ({
            value: field.code,
            label: field.title
          }))}
          onChange={(target_field) => onChange({ ...rule, target_field })}
        />
        <Select
          aria-label={i18nText('frontstage', 'data_blocks.form.rule_effect')}
          style={{ minWidth: 140 }}
          getPopupContainer={popupContainer}
          value={rule.effect}
          options={[
            {
              value: 'show',
              label: i18nText('frontstage', 'data_blocks.form.show')
            },
            {
              value: 'hide',
              label: i18nText('frontstage', 'data_blocks.form.hide')
            },
            {
              value: 'required',
              label: i18nText('frontstage', 'data_blocks.fields.required')
            },
            {
              value: 'optional',
              label: i18nText('frontstage', 'data_blocks.form.optional')
            },
            {
              value: 'readonly',
              label: i18nText('frontstage', 'data_blocks.form.readonly')
            },
            {
              value: 'editable',
              label: i18nText('frontstage', 'data_blocks.form.editable')
            }
          ]}
          onChange={(effect) => onChange({ ...rule, effect })}
        />
        <Button
          icon={<DeleteOutlined />}
          danger
          onClick={onRemove}
          aria-label={i18nText('frontstage', 'data_blocks.form.remove_rule')}
        />
      </Space>
      <Divider />
    </section>
  );
}
