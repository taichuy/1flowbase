import type { ConsoleDataModel } from '@1flowbase/api-client';
import { Button, Form, Input, Select, Space } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';

export function DataFieldTypeOptions({
  kind,
  models
}: {
  kind: string;
  models: ConsoleDataModel[];
}) {
  return (
    <>
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
            options={models.map((item) => ({
              value: item.id,
              label: item.title
            }))}
          />
        </Form.Item>
      ) : null}
    </>
  );
}
