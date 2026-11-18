import { Checkbox, Form, Input, Select } from 'antd';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../../shared/i18n/text';
import type {
  DataBlockField,
  DataFormItem,
  FormBlockConfig
} from '../../../../lib/data-blocks/types';
import { useDataBlockPopupContainer } from '../../shared/useDataBlockPopupContainer';
import { FormFieldControl } from '../FormFieldControl';
import type { DesignDialog } from '../FormDesignControls';

export function FormItemSettingsFields({
  ctx,
  dialog,
  draft,
  item,
  field,
  patch,
  patchItem
}: {
  ctx: BlockContext;
  dialog: Exclude<DesignDialog, null>;
  draft: FormBlockConfig;
  item: DataFormItem | null;
  field: DataBlockField | undefined;
  patch(values: Partial<FormBlockConfig>): void;
  patchItem(values: Partial<DataFormItem>): void;
}) {
  const popupContainer = useDataBlockPopupContainer();
  return (
    <>
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
                label={i18nText('frontstage', 'data_blocks.form.default_value')}
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
    </>
  );
}
