import { Form, Input, InputNumber } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import { parseResponseFields } from './tool-editor-model';

export function McpToolReturnDefaultsFields() {
  return (
    <>
      <Form.Item
        name="max_inline_chars"
        label={i18nText('settingsMcpManagement', 'auto.return_budget')}
        extra={i18nText('settingsMcpManagement', 'auto.return_budget_help')}
        rules={[
          {
            validator: async (_, value: number | null | undefined) => {
              if (
                value != null &&
                (!Number.isSafeInteger(value) || value < 1)
              ) {
                throw new Error(
                  i18nText(
                    'settingsMcpManagement',
                    'auto.return_budget_invalid'
                  )
                );
              }
            }
          }
        ]}
      >
        <InputNumber min={1} precision={0} style={{ width: '100%' }} />
      </Form.Item>
      <Form.Item
        name="response_fields"
        label={i18nText('settingsMcpManagement', 'auto.return_fields')}
        extra={i18nText('settingsMcpManagement', 'auto.return_fields_help')}
        rules={[
          {
            validator: async (_, value: string | undefined) => {
              try {
                parseResponseFields(value);
              } catch {
                throw new Error(
                  i18nText(
                    'settingsMcpManagement',
                    'auto.return_fields_invalid'
                  )
                );
              }
            }
          }
        ]}
      >
        <Input placeholder='["/title", "/body"]' />
      </Form.Item>
    </>
  );
}
