import { Form, Input, Select, Typography } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import type {
  DataBlockField,
  DataBlockRelation
} from '../../../lib/data-blocks/types';

/** A relation binds record identity through a backend-declared field, never block ancestry. */
export function DataRelationEditor({
  value,
  onChange,
  fields
}: {
  value: DataBlockRelation | undefined;
  onChange(value: DataBlockRelation | undefined): void;
  fields: readonly DataBlockField[];
}) {
  const candidates = fields.filter(
    (field) =>
      field.field_kind === 'many_to_one' && field.relation_target_model_code
  );
  return (
    <>
      <Form.Item label={i18nText('frontstage', 'data_blocks.relation.field')}>
        <Select
          allowClear
          value={value?.field_code}
          options={candidates.map((field) => ({
            label: field.title,
            value: field.code
          }))}
          onChange={(fieldCode: string | undefined) => {
            const field = candidates.find((item) => item.code === fieldCode);
            onChange(
              field?.relation_target_model_code
                ? {
                    field_code: field.code,
                    parent_model_code: field.relation_target_model_code,
                    parent_record_input:
                      value?.parent_record_input || 'parent_record_id'
                  }
                : undefined
            );
          }}
        />
      </Form.Item>
      {value ? (
        <Form.Item
          label={i18nText('frontstage', 'data_blocks.relation.parent_input')}
        >
          <Input
            value={value.parent_record_input}
            onChange={(event) => {
              if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(event.target.value))
                onChange({ ...value, parent_record_input: event.target.value });
            }}
          />
        </Form.Item>
      ) : null}
      <Typography.Paragraph type="secondary">
        {i18nText('frontstage', 'data_blocks.relation.explanation')}
      </Typography.Paragraph>
    </>
  );
}
