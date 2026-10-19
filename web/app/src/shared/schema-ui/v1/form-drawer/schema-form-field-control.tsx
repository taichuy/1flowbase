import { Input, InputNumber, Select, Switch } from 'antd';

import type { PluginFormFieldSchema } from '../contracts/plugin-form-schema';

export function renderSchemaFormFieldControl(field: PluginFormFieldSchema) {
  if (field.type === 'number') {
    return (
      <InputNumber
        className="schema-form-drawer__number"
        max={field.max}
        min={field.min}
        precision={field.precision}
        step={field.step}
      />
    );
  }

  if (field.type === 'boolean') {
    return <Switch />;
  }

  if (field.type === 'select' || field.control === 'multi-select') {
    return (
      <Select
        mode={field.control === 'multi-select' ? 'multiple' : undefined}
        options={(field.options ?? []).map((option) => ({
          disabled: option.disabled,
          label: option.label,
          value: option.value as string | number | boolean
        }))}
      />
    );
  }

  if (
    field.control === 'password' ||
    field.type === 'password' ||
    field.type === 'secret'
  ) {
    return (
      <Input.Password
        placeholder={field.placeholder}
        autoComplete="new-password"
      />
    );
  }

  if (field.control === 'textarea') {
    return (
      <Input.TextArea
        autoSize={{ minRows: 3, maxRows: 8 }}
        placeholder={field.placeholder}
      />
    );
  }

  return <Input placeholder={field.placeholder} />;
}
