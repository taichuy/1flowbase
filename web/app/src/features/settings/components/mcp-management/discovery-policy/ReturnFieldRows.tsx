import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { Button, Input } from 'antd';
import { useStableListItemKeys } from '../../../../agent-flow/hooks/interactions/use-stable-list-item-keys';
import { i18nText } from '../../../../../shared/i18n/text';

export function ReturnFieldRows({ fields, fieldLabel, onChange }: {
  fields: string[];
  fieldLabel: string;
  onChange: (fields: string[]) => void;
}) {
  const rowKeys = useStableListItemKeys('mcp-return-field', fields.length);
  return (
    <div className="mcp-management__return-fields">
      <div className="mcp-management__return-field-rows">
        {fields.map((field, index) => (
          <div key={rowKeys.itemKeys[index]} className="mcp-management__return-field-row">
            <Input
              aria-label={`${fieldLabel} ${index + 1}`}
              value={field}
              onChange={(event) => {
                const nextFields = [...fields];
                nextFields[index] = event.target.value;
                onChange(nextFields);
              }}
            />
            <Button
              danger
              type="text"
              aria-label={i18nText(
                'settingsMcpManagement',
                'auto.delete_list_return_field',
                { value1: field || index + 1 }
              )}
              icon={<DeleteOutlined />}
              onClick={() => {
                rowKeys.removeItemKey(index);
                onChange(fields.filter((_, fieldIndex) => fieldIndex !== index));
              }}
            />
          </div>
        ))}
      </div>
      <Button
        block
        type="dashed"
        icon={<PlusOutlined />}
        onClick={() => {
          rowKeys.insertItemKey();
          onChange([...fields, '']);
        }}
      >
        {i18nText('settingsMcpManagement', 'auto.add_list_return_field')}
      </Button>
    </div>
  );
}
