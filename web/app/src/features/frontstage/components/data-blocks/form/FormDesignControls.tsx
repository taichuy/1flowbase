import { BlockDesignButton } from '../../../lib/native-modules/block-design/runtime';
import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { DataBlockModal as Modal } from '../shared/DataBlockModal';
import { useState, type ReactNode } from 'react';
import { Checkbox, Dropdown, Form, Input, Switch } from 'antd';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  type DataBlockField,
  type DataFormItem,
  type FormBlockConfig
} from '../../../lib/data-blocks/types';
import { DataRelationEditor } from '../shared/DataRelationEditor';
import { DataFieldCreateModal } from '../shared/DataFieldCreateModal';
import { newFormItem } from './form-state';
import { FormItemSettingsFields } from './design-settings/FormItemSettingsFields';
import { FormLayoutSettingsFields } from './design-settings/FormLayoutSettingsFields';
import { FormRuleSettingsFields } from './design-settings/FormRuleSettingsFields';

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

export function FormDesignControls(props: Props) {
  const popupContainer = useDataBlockPopupContainer();
  const { ctx, config, fields, saving, onSave, dialog, setDialog } = props;
  const [creatingField, setCreatingField] = useState(false);
  const [search, setSearch] = useState('');
  const fieldMenu = fields.flatMap((field) => {
    if (
      !`${field.title} ${field.code}`
        .toLowerCase()
        .includes(search.toLowerCase())
    )
      return [];
    return [
      {
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
      }
    ];
  });
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
          <BlockDesignButton
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
          <BlockDesignButton variant="initializer" disabled={saving}>
            {i18nText('frontstage', 'data_blocks.form.configure_fields')}
          </BlockDesignButton>
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
          <BlockDesignButton variant="initializer" disabled={saving}>
            {i18nText('frontstage', 'data_blocks.form.configure_actions')}
          </BlockDesignButton>
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
        <FormItemSettingsFields
          ctx={ctx}
          dialog={dialog}
          draft={draft}
          item={item}
          field={field}
          patch={patch}
          patchItem={patchItem}
        />
        {dialog === 'layout' && (
          <FormLayoutSettingsFields draft={draft} patch={patch} />
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
          <FormRuleSettingsFields draft={draft} fields={fields} patch={patch} />
        )}
      </Form>
    </Modal>
  );
}
