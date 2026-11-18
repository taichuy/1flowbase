import { Checkbox, Form, InputNumber, Radio, Select } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import type { FormBlockConfig } from '../../../../lib/data-blocks/types';
import { useDataBlockPopupContainer } from '../../shared/useDataBlockPopupContainer';

export function FormLayoutSettingsFields({
  draft,
  patch
}: {
  draft: FormBlockConfig;
  patch(values: Partial<FormBlockConfig>): void;
}) {
  const popupContainer = useDataBlockPopupContainer();
  return (
    <>
      <Form.Item>
        <Radio.Group
          value={draft.label_layout}
          onChange={(event) => patch({ label_layout: event.target.value })}
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
              onChange={(event) => patch({ label_wrap: event.target.checked })}
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
  );
}
