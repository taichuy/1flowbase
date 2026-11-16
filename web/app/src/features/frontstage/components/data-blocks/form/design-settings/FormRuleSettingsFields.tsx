import { Button, Divider, Select, Space, Typography } from 'antd';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { i18nText } from '../../../../../../shared/i18n/text';
import {
  emptyDataFilter,
  type DataBlockField,
  type DataFormRule,
  type FormBlockConfig
} from '../../../../lib/data-blocks/types';
import { useDataBlockPopupContainer } from '../../shared/useDataBlockPopupContainer';
import { DataBlockFilterEditor } from '../../shared/DataBlockFilterEditor';

export function FormRuleSettingsFields({
  draft,
  fields,
  patch
}: {
  draft: Pick<FormBlockConfig, 'rules'>;
  fields: DataBlockField[];
  patch(values: Pick<FormBlockConfig, 'rules'>): void;
}) {
  return (
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
              rules: draft.rules.filter((candidate) => candidate.id !== rule.id)
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
  const popupContainer = useDataBlockPopupContainer();
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
